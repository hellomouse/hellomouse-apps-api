use crate::music::handlers::postgres_handler::PostgresHandler;
use crate::shared::types::app::{ErrorResponse, Response, login_fail, no_update_permission, no_view_permission};
use crate::music::types::{Playlist, PlaylistDetails, Song, SongAbridged};
use crate::shared::types::account::Perm;
use uuid::Uuid;

use std::collections::HashMap;
use actix_identity::Identity;
use actix_web::{
    get, post, delete, put, HttpResponse, web::{self, Data}, Result
};
use serde::{Serialize, Deserialize};


// Add playlist
#[derive(Deserialize)]
struct AddPlaylistForm {
    name: String
}

#[derive(Serialize)]
struct AddPlaylistReturn { id: Uuid }

#[post("/v1/music/playlist")]
async fn create_playlist(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<AddPlaylistForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.create_playlist(
            identity.id().unwrap().as_str(),
            params.name.as_str()
        ).await {
            Ok(result) => Ok(HttpResponse::Ok().json(AddPlaylistReturn { id: result })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error creating playlist".to_string() }))
        };
    }
    login_fail!();
}


// Edit playlist
#[derive(Deserialize)]
struct EditPlaylistForm {
    name: String,
    id: Uuid
}

#[put("/v1/music/playlist")]
async fn edit_playlist(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<EditPlaylistForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        if !handler.is_user_owner_playlist(identity.id().unwrap().as_str(), &params.id).await.unwrap() {
            no_update_permission!();
        }

        return match handler.edit_playlist(
            &params.id,
            params.name.as_str()
        ).await {
            Ok(_result) => Ok(HttpResponse::Ok().json(Response { msg: "Playlist updated".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error updating playlist".to_string() }))
        };
    }
    login_fail!();
}


// Edit playlist perms
#[derive(Deserialize)]
struct EditPlaylistPermsForm {
    perms: HashMap<String, Perm>,
    id: Uuid
}

#[put("/v1/music/playlist/perms")]
async fn edit_playlist_perms(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<EditPlaylistPermsForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        if !handler.can_user_edit_playlist(identity.id().unwrap().as_str(), &params.id).await.unwrap() {
            no_update_permission!();
        }

        return match handler.edit_playlist_perms(
            identity.id().unwrap().as_str(),
            &params.id,
            params.perms.clone()
        ).await {
            Ok(_result) => Ok(HttpResponse::Ok().json(Response { msg: "Playlist perms updated".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error updating playlist perms".to_string() }))
        };
    }
    login_fail!();
}


// Delete playlist
#[derive(Deserialize)]
struct RemovePlaylistForm {
    id: Uuid
}

#[delete("/v1/music/playlist")]
async fn delete_playlist(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<RemovePlaylistForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        if !handler.is_user_owner_playlist(identity.id().unwrap().as_str(), &params.id).await.unwrap() {
            no_update_permission!();
        }

        return match handler.delete_playlist(&params.id).await {
            Ok(_result) => Ok(HttpResponse::Ok().json(Response { msg: "Playlist removed".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error deleting playlist".to_string() }))
        };
    }
    login_fail!();
}


// Get single playlist
#[derive(Deserialize)]
struct GetPlaylistForm { id: Uuid }

#[derive(Serialize)]
struct GetPlaylistReturn { playlist: PlaylistDetails }

#[get("/v1/music/playlist/single")]
async fn get_playlist(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Query<GetPlaylistForm>) -> Result<HttpResponse> {
    let mut user_id = "public".to_string();
    if let Some(identity) = identity {
        user_id = identity.id().unwrap().to_owned();
    }

    if !handler.can_user_view_playlist(&user_id, &params.id).await.unwrap() {
        no_view_permission!();
    }

    return match handler.get_playlist(&user_id, &params.id).await {
        Ok(result) => Ok(HttpResponse::Ok().json(GetPlaylistReturn { playlist: result })),
        Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error getting playlist".to_string() }))
    };
}


// Get playlists
#[derive(Serialize)]
struct GetPlaylistsReturn {
    playlists: Vec<Playlist>
}

#[get("/v1/music/playlist")]
async fn get_playlists(handler: Data<PostgresHandler>, identity: Option<Identity>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.get_playlists(identity.id().unwrap().as_str()).await {
            Ok(result) => Ok(HttpResponse::Ok().json(GetPlaylistsReturn { playlists: result })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error getting playlists".to_string() }))
        };
    }
    login_fail!();
}

// Add to user playlist
#[derive(Deserialize)]
struct UserPlaylistForm { id: Uuid }

#[post("/v1/music/user_playlist")]
async fn add_user_playlist(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<UserPlaylistForm>) -> Result<HttpResponse> {
    let mut user_id = "public".to_string();
    if let Some(identity) = identity {
        user_id = identity.id().unwrap().to_owned();
    }

    if !handler.can_user_view_playlist(&user_id, &params.id).await.unwrap() {
        no_view_permission!();
    }

    return match handler.add_to_user_playlists(&user_id, &params.id).await {
        Ok(_result) => Ok(HttpResponse::Ok().json(Response { msg: "Added".to_string() })),
        Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error adding to user playlists".to_string() }))
    };
}

#[delete("/v1/music/user_playlist")]
async fn remove_user_playlist(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<UserPlaylistForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.delete_from_user_playlists(identity.id().unwrap().as_str(), &params.id).await {
            Ok(_result) => Ok(HttpResponse::Ok().json(Response { msg: "Deleted".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error deleting from user playlists".to_string() }))
        };
    }
    login_fail!();
}




// Add songs to playlist by URL
#[derive(Deserialize)]
struct AddUrlsToPlaylistForm {
    id: Uuid,
    song_urls: Vec<String>
}

#[post("/v1/music/playlist/song/url")]
async fn add_songs_by_url(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<AddUrlsToPlaylistForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        if !handler.can_add_songs(identity.id().unwrap().as_str(), params.song_urls.len()).await.unwrap() {
            return Ok(HttpResponse::TooManyRequests().json(ErrorResponse{ error: "You already have too many urls queued, try again later".to_string() }))
        }

        return match handler.add_songs_by_url(
            identity.id().unwrap().as_str(),
            &params.id,
            &params.song_urls
        ).await {
            Ok(_result) => Ok(HttpResponse::Ok().json(Response { msg: "Song urls queued".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error adding song urls".to_string() }))
        };
    }
    login_fail!();
}

// Get songs in playlist
#[derive(Deserialize)]
struct GetPlaylistSongsForm {
    id: Uuid,
    offset: Option<i32>,
    limit: Option<i32>
}

#[derive(Serialize)]
struct GetPlaylistSongsReturn {
    songs: Vec<SongAbridged>
}

#[get("/v1/music/playlist/song")]
async fn get_songs(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Query<GetPlaylistSongsForm>) -> Result<HttpResponse> {
    let mut user_id = "public".to_string();
    if let Some(identity) = identity {
        user_id = identity.id().unwrap().to_owned();
    }

    if !handler.can_user_view_playlist(&user_id, &params.id).await.unwrap() {
        no_view_permission!();
    }

    return match handler.get_songs(
        &params.id,
        params.offset,
        params.limit
    ).await {
        Ok(result) => Ok(HttpResponse::Ok().json(GetPlaylistSongsReturn { songs: result })),
        Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error getting songs".to_string() }))
    };
}

// Get song
#[derive(Deserialize)]
struct GetSongForm { id: String }

#[derive(Serialize)]
struct GetSongReturn { song: Option<Song> }

#[get("/v1/music/song")]
async fn get_song(handler: Data<PostgresHandler>, params: web::Query<GetSongForm>) -> Result<HttpResponse> {
    return match handler.get_song(&params.id).await {
        Ok(result) => Ok(HttpResponse::Ok().json(GetSongReturn { song: result })),
        Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error getting song".to_string() }))
    };
}
