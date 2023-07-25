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
        return Ok(HttpResponse::Unauthorized().json(ErrorResponse { error: "Unauthorized".to_string() })) }
}

macro_rules! no_update_permission {
    () => { return Ok(HttpResponse::Unauthorized().json(
            ErrorResponse{ error: "You do not have permission to update this resource".to_string() })) }
}

macro_rules! no_view_permission {
    () => { return Ok(HttpResponse::Unauthorized().json(
            ErrorResponse{ error: "You do not have permission to view this resource".to_string() })) }
}

pub(crate) use login_fail;
pub(crate) use no_update_permission;
pub(crate) use no_view_permission;
