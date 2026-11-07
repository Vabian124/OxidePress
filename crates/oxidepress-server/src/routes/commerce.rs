// Modified by Vabian124 on 2026-11-07 to reduce product stock when an order is placed.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
use axum::{
    extract::{Form, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json, Redirect, Response},
    routing::{get, post},
    Router,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::Deserialize;
use std::sync::Arc;

use oxidepress_commerce::woo_compat::{self, WooOrderData, WooProductData};
use oxidepress_db::entities::wp_posts;
use oxidepress_db::queries;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct AddToCartRequest {
    pub product_id: u64,
    pub quantity: Option<u32>,
}

#[derive(Deserialize)]
pub struct CheckoutFormData {
    pub name: String,
    pub email: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub zip: String,
    pub country: String,
    pub payment_method: String,
}

#[derive(Deserialize)]
struct CartForm {
    product_id: u64,
    quantity: Option<u32>,
}

const CART_COOKIE: &str = "oxidepress_cart";

fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    let prefix = format!("{name}=");
    raw.split(';').find_map(|part| {
        let part = part.trim();
        part.strip_prefix(&prefix)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
    })
}

fn cart_identity(headers: &HeaderMap) -> (String, bool) {
    if let Some(id) = read_cookie(headers, CART_COOKIE) {
        return (id, false);
    }
    (uuid::Uuid::new_v4().simple().to_string(), true)
}

fn set_cart_cookie(response: &mut Response, id: &str) {
    let value = format!("{CART_COOKIE}={id}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000");
    if let Ok(value) = header::HeaderValue::from_str(&value) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
}

fn esc(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn shop_markup(body: &str) -> String {
    format!(
        r#"<style>
.oxide-shop .products {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 1.25rem; }}
.oxide-shop .product, .oxide-shop .single-product {{ padding: 1rem; border: 1px solid #dcdcde; }}
.oxide-shop img {{ width: 100%; height: auto; display: block; }}
.oxide-shop .price del {{ color: #646970; margin-right: 0.4rem; }}
.oxide-shop table {{ width: 100%; border-collapse: collapse; }}
.oxide-shop th, .oxide-shop td {{ padding: 0.6rem; border-bottom: 1px solid #dcdcde; text-align: left; }}
.oxide-shop button, .oxide-shop .button {{ background: #111; color: #fff; border: 0; padding: 0.6rem 1rem; cursor: pointer; text-decoration: none; display: inline-block; }}
.oxide-shop .checkout-form {{ display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }}
.oxide-shop .checkout-form label {{ display: block; font-weight: 600; margin-bottom: 0.25rem; }}
.oxide-shop .checkout-form input, .oxide-shop .checkout-form select {{ width: 100%; padding: 0.5rem; box-sizing: border-box; }}
.oxide-shop .full-width {{ grid-column: 1 / -1; }}
</style>
<div class="oxide-shop">{body}</div>"#
    )
}

async fn present(state: &AppState, title: &str, body_class: &str, body: &str) -> Html<String> {
    super::frontend::render_themed_document(state, title, body_class, &shop_markup(body)).await
}

async fn published_products(state: &AppState) -> Vec<woo_compat::WooProductData> {
    let posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(woo_compat::post_types::PRODUCT))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .order_by_desc(wp_posts::Column::PostDate)
        .all(&state.db)
        .await
        .unwrap_or_default();
    let mut products = Vec::new();
    for post in posts {
        let meta = queries::get_post_meta_map(&state.db, post.id)
            .await
            .unwrap_or_default();
        products.push(woo_compat::WooProductData::from_post_and_meta(
            post.id,
            &post.post_title,
            &post.post_name,
            &post.post_content,
            &post.post_excerpt,
            &meta,
        ));
    }
    products
}

fn price_html(product: &woo_compat::WooProductData) -> String {
    match product.sale_price {
        Some(sale) if sale > 0.0 && sale < product.regular_price => format!(
            r#"<p class="price"><del>${:.2}</del><ins>${:.2}</ins></p>"#,
            product.regular_price, sale
        ),
        _ => format!(r#"<p class="price">${:.2}</p>"#, product.price),
    }
}

fn picture_html(product: &woo_compat::WooProductData) -> String {
    if woo_compat::image_url_is_safe(&product.image_url) && !product.image_url.is_empty() {
        format!(r#"<img src="{}" alt="">"#, esc(&product.image_url))
    } else {
        String::new()
    }
}

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/shop", get(shop_page))
        .route("/shop/product/{id}", get(product_page))
        .route("/cart", get(cart_page))
        .route("/cart/add", post(cart_add_form))
        .route("/checkout", get(checkout_page))
        .route("/checkout/thank-you/{order_id}", get(thank_you_page))
        .route("/api/cart/add", post(cart_add))
        .route("/api/cart/remove/{product_id}", post(cart_remove))
        .route("/api/cart", get(cart_get))
        .route("/api/checkout", post(checkout_submit))
        .route("/api/orders", get(orders_list))
        .route("/api/orders/{id}", get(order_detail))
        .route("/api/products", get(products_list))
        .route("/api/products/{id}", get(product_detail))
        .with_state(state)
}

async fn shop_page(State(state): State<Arc<AppState>>) -> Html<String> {
    let products = published_products(&state).await;
    let mut body = String::from("<h1>Shop</h1>");
    if products.is_empty() {
        body.push_str("<p>No products are published yet.</p>");
    } else {
        body.push_str(r#"<div class="products">"#);
        for product in products {
            body.push_str(&format!(
                r#"<article class="product">{picture}<h2><a href="/shop/product/{id}">{name}</a></h2>{price}<p>{summary}</p></article>"#,
                picture = picture_html(&product),
                id = product.post_id,
                name = esc(&product.name),
                price = price_html(&product),
                summary = esc(&product.short_description),
            ));
        }
        body.push_str("</div>");
    }
    present(&state, "Shop", "oxide-shop", &body).await
}

async fn product_page(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Html<String>, StatusCode> {
    let product = published_products(&state)
        .await
        .into_iter()
        .find(|product| product.post_id == id)
        .ok_or(StatusCode::NOT_FOUND)?;
    let body = format!(
        r#"<article class="single-product">{picture}<h1>{name}</h1>{price}<div class="description">{description}</div>
<form method="post" action="/cart/add">
  <input type="hidden" name="product_id" value="{id}">
  <label>Quantity <input type="number" name="quantity" value="1" min="1" style="width:4rem"></label>
  <button type="submit">Add to cart</button>
</form>
<p><a href="/shop">Back to the shop</a></p>
</article>"#,
        picture = picture_html(&product),
        name = esc(&product.name),
        price = price_html(&product),
        description = esc(&product.description),
        id = product.post_id,
    );
    Ok(present(&state, &product.name, "oxide-product", &body).await)
}

async fn add_product_to_cart(state: &AppState, key: &str, product_id: u64, quantity: u32) -> bool {
    let product = published_products(state)
        .await
        .into_iter()
        .find(|product| product.post_id == product_id);
    let Some(product) = product else {
        return false;
    };
    let mut cart_mgr = state.cart_manager.write().await;
    let cart = cart_mgr.get_or_create_cart(key);
    cart.add_item(oxidepress_commerce::CartItem {
        product_id,
        name: product.name.clone(),
        price: product.price,
        quantity,
        variation_id: None,
    });
    true
}

async fn cart_add_form(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(form): Form<CartForm>,
) -> Response {
    let (key, is_new) = cart_identity(&headers);
    let quantity = form.quantity.unwrap_or(1).max(1);
    let found = add_product_to_cart(&state, &key, form.product_id, quantity).await;
    let target = if found { "/cart" } else { "/shop" };
    let mut response = Redirect::to(target).into_response();
    if is_new {
        set_cart_cookie(&mut response, &key);
    }
    response
}

async fn cart_page(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Html<String> {
    let (key, _) = cart_identity(&headers);
    let cart_mgr = state.cart_manager.read().await;
    let body = if let Some(cart) = cart_mgr
        .get_cart(&key)
        .filter(|cart| !cart.items.is_empty())
    {
        let mut rows = String::new();
        for item in &cart.items {
            rows.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>${:.2}</td><td>${:.2}</td></tr>",
                esc(&item.name),
                item.quantity,
                item.price,
                item.price * item.quantity as f64
            ));
        }
        format!(
            r#"<h1>Cart</h1>
<table><thead><tr><th>Product</th><th>Qty</th><th>Price</th><th>Total</th></tr></thead><tbody>{rows}</tbody>
<tfoot><tr><td colspan="3">Total</td><td>${total:.2}</td></tr></tfoot></table>
<p><a class="button" href="/checkout">Checkout</a></p>"#,
            total = cart.get_total()
        )
    } else {
        r#"<h1>Cart</h1><p>Your cart is empty.</p><p><a href="/shop">Continue shopping</a></p>"#
            .to_string()
    };
    present(&state, "Cart", "oxide-cart", &body).await
}

async fn cart_add(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<AddToCartRequest>,
) -> Response {
    let (key, is_new) = cart_identity(&headers);
    let quantity = req.quantity.unwrap_or(1).max(1);
    let found = add_product_to_cart(&state, &key, req.product_id, quantity).await;
    let (status, body) = if found {
        (
            StatusCode::OK,
            serde_json::json!({"status": "added", "product_id": req.product_id, "quantity": quantity}),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            serde_json::json!({"error": "Product not found"}),
        )
    };
    let mut response = (status, Json(body)).into_response();
    if is_new {
        set_cart_cookie(&mut response, &key);
    }
    response
}

async fn cart_remove(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(product_id): Path<u64>,
) -> impl IntoResponse {
    let (key, _) = cart_identity(&headers);
    let mut cart_mgr = state.cart_manager.write().await;
    if let Some(cart) = cart_mgr.get_cart_mut(&key) {
        cart.remove_item(product_id, None);
    }
    Json(serde_json::json!({"status": "removed", "product_id": product_id}))
}

async fn cart_get(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    let (key, _) = cart_identity(&headers);
    let cart_mgr = state.cart_manager.read().await;
    if let Some(cart) = cart_mgr.get_cart(&key) {
        Json(serde_json::json!({
            "items": cart.items,
            "total": cart.get_total()
        }))
    } else {
        Json(serde_json::json!({"items": Vec::<()>::new(), "total": 0.0}))
    }
}

async fn products_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let products = published_products(&state).await;
    Json(serde_json::json!(products))
}

async fn product_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, StatusCode> {
    let product = published_products(&state)
        .await
        .into_iter()
        .find(|product| product.post_id == id)
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!(product)))
}

async fn checkout_page(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Html<String> {
    let (key, _) = cart_identity(&headers);
    let cart_mgr = state.cart_manager.read().await;
    let (items_html, total) = if let Some(cart) = cart_mgr.get_cart(&key) {
        let mut html = String::new();
        for item in &cart.items {
            let line_total = item.price * item.quantity as f64;
            html.push_str(&format!(
                r#"<tr><td>{}</td><td>{}</td><td>${:.2}</td><td>${:.2}</td></tr>"#,
                esc(&item.name),
                item.quantity,
                item.price,
                line_total
            ));
        }
        (html, cart.get_total())
    } else {
        (String::new(), 0.0)
    };

    let body = format!(
        r#"<h1>Checkout</h1>
{cart_section}"#,
        cart_section = if total == 0.0 {
            r#"<p>Your cart is empty. <a href="/shop">Continue shopping</a>.</p>"#.to_string()
        } else {
            format!(
                r#"<h2>Order Summary</h2>
<table>
<thead><tr><th>Product</th><th>Qty</th><th>Price</th><th>Total</th></tr></thead>
<tbody>{items_html}</tbody>
<tfoot><tr><td colspan="3">Total</td><td>${total:.2}</td></tr></tfoot>
</table>
<h2>Billing Details</h2>
<form id="checkout-form" class="checkout-form">
  <div><label for="name">Full Name</label><input type="text" id="name" name="name" required></div>
  <div><label for="email">Email</label><input type="email" id="email" name="email" required></div>
  <div class="full-width"><label for="address">Address</label><input type="text" id="address" name="address" required></div>
  <div><label for="city">City</label><input type="text" id="city" name="city" required></div>
  <div><label for="state">State / Province</label><input type="text" id="state" name="state" required></div>
  <div><label for="zip">ZIP / Postal Code</label><input type="text" id="zip" name="zip" required></div>
  <div><label for="country">Country</label><input type="text" id="country" name="country" required value="US"></div>
  <div class="full-width">
    <label for="payment_method">Payment Method</label>
    <select id="payment_method" name="payment_method" required>
      <option value="credit_card">Credit Card</option>
      <option value="paypal">PayPal</option>
      <option value="bank_transfer">Bank Transfer</option>
    </select>
  </div>
  <div class="full-width">
    <button type="submit">Place Order</button>
  </div>
</form>
<script>
document.getElementById('checkout-form').addEventListener('submit', function(e) {{
  e.preventDefault();
  const form = e.target;
  const data = {{
    name: form.name.value,
    email: form.email.value,
    address: form.address.value,
    city: form.city.value,
    state: form.state.value,
    zip: form.zip.value,
    country: form.country.value,
    payment_method: form.payment_method.value
  }};
  fetch('/api/checkout', {{
    method: 'POST',
    headers: {{'Content-Type': 'application/json'}},
    body: JSON.stringify(data)
  }})
  .then(r => r.json())
  .then(d => {{
    if (d.order_id) {{
      window.location.href = '/checkout/thank-you/' + d.order_id;
    }} else {{
      alert(d.error || 'Checkout failed');
    }}
  }})
  .catch(err => alert('Error: ' + err));
}});
</script>"#
            )
        }
    );
    present(&state, "Checkout", "oxide-checkout", &body).await
}

async fn checkout_submit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(form): Json<CheckoutFormData>,
) -> impl IntoResponse {
    let (session_id, _) = cart_identity(&headers);

    // Read cart items
    let cart_mgr = state.cart_manager.read().await;
    let cart = match cart_mgr.get_cart(&session_id) {
        Some(c) if !c.items.is_empty() => c.clone(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Cart is empty"})),
            );
        }
    };
    drop(cart_mgr);

    // Build order items from cart
    let order_items: Vec<oxidepress_commerce::OrderItem> = cart
        .items
        .iter()
        .map(|item| oxidepress_commerce::OrderItem {
            product_id: item.product_id,
            name: item.name.clone(),
            quantity: item.quantity,
            price: item.price,
            total: item.price * item.quantity as f64,
        })
        .collect();

    // Parse name into first/last
    let name_parts: Vec<&str> = form.name.splitn(2, ' ').collect();
    let first_name = name_parts.first().unwrap_or(&"").to_string();
    let last_name = if name_parts.len() > 1 {
        name_parts[1].to_string()
    } else {
        String::new()
    };

    let billing = oxidepress_commerce::Address {
        first_name: first_name.clone(),
        last_name: last_name.clone(),
        company: String::new(),
        address_1: form.address.clone(),
        address_2: String::new(),
        city: form.city.clone(),
        state: form.state.clone(),
        postcode: form.zip.clone(),
        country: form.country.clone(),
        email: form.email.clone(),
        phone: String::new(),
    };

    // Create order
    let mut order_mgr = state.order_manager.write().await;
    let order_id = order_mgr.create_order(
        order_items,
        billing.clone(),
        billing.clone(),
        &form.payment_method,
        0.0, // shipping_total
        0.0, // tax_total
        0.0, // discount_total
        None,
        "",
    );

    let order = order_mgr.get_order(order_id).unwrap();
    let total = order.total;
    let status = order.status.to_string();

    // Mark as processing (payment stub succeeds)
    order_mgr.update_status(order_id, oxidepress_commerce::OrderStatus::Processing);
    drop(order_mgr);

    let stored = WooOrderData {
        order_key: format!("wc_order_{order_id}"),
        status: "wc-processing".to_string(),
        currency: "USD".to_string(),
        total,
        payment_method: form.payment_method.clone(),
        payment_method_title: form.payment_method.clone(),
        billing: billing.clone(),
        shipping: billing.clone(),
        ..WooOrderData::default()
    };
    let now = chrono::Utc::now().naive_utc();
    let order_post = wp_posts::ActiveModel {
        post_author: Set(0),
        post_date: Set(now),
        post_date_gmt: Set(now),
        post_content: Set(String::new()),
        post_title: Set(format!("Order {order_id}")),
        post_excerpt: Set(String::new()),
        post_status: Set("wc-processing".to_string()),
        comment_status: Set("closed".to_string()),
        ping_status: Set("closed".to_string()),
        post_password: Set(String::new()),
        post_name: Set(format!("order-{order_id}")),
        to_ping: Set(String::new()),
        pinged: Set(String::new()),
        post_modified: Set(now),
        post_modified_gmt: Set(now),
        post_content_filtered: Set(String::new()),
        post_parent: Set(0),
        guid: Set(String::new()),
        menu_order: Set(0),
        post_type: Set(woo_compat::post_types::ORDER.to_string()),
        post_mime_type: Set(String::new()),
        comment_count: Set(0),
        ..Default::default()
    };
    if let Ok(inserted) = order_post.insert(&state.db).await {
        if let Err(error) =
            queries::set_post_meta_bulk(&state.db, inserted.id, &stored.to_meta()).await
        {
            tracing::error!(%error, "shop order meta was not saved");
        }
    }

    for item in &cart.items {
        let meta = queries::get_post_meta_map(&state.db, item.product_id)
            .await
            .unwrap_or_default();
        let product = WooProductData::from_post_and_meta(item.product_id, "", "", "", "", &meta);
        if let Some((next, status)) = product.after_sale(item.quantity as i64) {
            let _ = queries::set_post_meta(
                &state.db,
                item.product_id,
                woo_compat::product_keys::STOCK,
                &next.to_string(),
            )
            .await;
            let _ = queries::set_post_meta(
                &state.db,
                item.product_id,
                woo_compat::product_keys::STOCK_STATUS,
                status,
            )
            .await;
        }
    }

    // Clear the cart
    let mut cart_mgr = state.cart_manager.write().await;
    if let Some(cart) = cart_mgr.get_cart_mut(&session_id) {
        cart.clear();
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "order_id": order_id,
            "status": status,
            "total": total
        })),
    )
}

async fn thank_you_page(
    State(state): State<Arc<AppState>>,
    Path(order_id): Path<u64>,
) -> Result<Html<String>, StatusCode> {
    let order_mgr = state.order_manager.read().await;
    let order = order_mgr.get_order(order_id).ok_or(StatusCode::NOT_FOUND)?;

    let mut items_html = String::new();
    for item in &order.items {
        items_html.push_str(&format!(
            r#"<tr><td>{}</td><td>{}</td><td>${:.2}</td><td>${:.2}</td></tr>"#,
            item.name, item.quantity, item.price, item.total
        ));
    }

    let billing_name = format!(
        "{} {}",
        order.billing_address.first_name, order.billing_address.last_name
    );

    let html = format!(
        r#"<h1>Thank you for your order</h1>
<p><strong>Order number:</strong> {order_number}<br>
<strong>Status:</strong> {status}<br>
<strong>Payment:</strong> {payment_method}<br>
<strong>Date:</strong> {date}</p>
<h2>Order details</h2>
<table>
<thead><tr><th>Product</th><th>Qty</th><th>Price</th><th>Total</th></tr></thead>
<tbody>{items_html}</tbody>
<tfoot><tr><td colspan="3">Order total</td><td>${total:.2}</td></tr></tfoot>
</table>
<h2>Billing address</h2>
<p>{billing_name}<br>{billing_address}<br>{billing_city}, {billing_state} {billing_zip}<br>{billing_country}<br>{billing_email}</p>
<p><a href="/shop">Continue shopping</a></p>"#,
        order_number = order.order_number,
        status = order.status,
        payment_method = order.payment_method,
        date = order.created_at.format("%B %e, %Y %H:%M"),
        items_html = items_html,
        total = order.total,
        billing_name = billing_name,
        billing_address = order.billing_address.address_1,
        billing_city = order.billing_address.city,
        billing_state = order.billing_address.state,
        billing_zip = order.billing_address.postcode,
        billing_country = order.billing_address.country,
        billing_email = order.billing_address.email,
    );

    Ok(present(&state, "Order confirmed", "oxide-order", &html).await)
}

async fn orders_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let order_mgr = state.order_manager.read().await;
    let orders = order_mgr.list_orders(None);
    let orders_json: Vec<serde_json::Value> = orders
        .iter()
        .map(|o| {
            serde_json::json!({
                "id": o.id,
                "order_number": o.order_number,
                "status": o.status,
                "total": o.total,
                "payment_method": o.payment_method,
                "created_at": o.created_at.to_rfc3339(),
                "item_count": o.items.len(),
            })
        })
        .collect();
    Json(serde_json::json!(orders_json))
}

async fn order_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, StatusCode> {
    let order_mgr = state.order_manager.read().await;
    let order = order_mgr.get_order(id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!(order)))
}
