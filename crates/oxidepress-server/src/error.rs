// Modified by Vabian124 on 2026-11-07 to rebrand as OxidePress.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use oxidepress_core::error::OxidePressError;

#[allow(dead_code)]
pub struct AppError(pub OxidePressError);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            OxidePressError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.0.to_string()).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError(OxidePressError::Internal(err.to_string()))
    }
}

impl From<OxidePressError> for AppError {
    fn from(err: OxidePressError) -> Self {
        AppError(err)
    }
}
