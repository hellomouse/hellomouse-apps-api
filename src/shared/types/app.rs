use serde::Serialize;

#[derive(Serialize)]
pub struct Response {
    pub msg: String
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String
}

macro_rules! login_fail {
    () => {
        return Ok(HttpResponse::Unauthorized().json(ErrorResponse { error: "Unauthorized".to_string() }))
    }
}

pub(crate) use login_fail;
