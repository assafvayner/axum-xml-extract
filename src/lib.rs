//! XML extractor for axum
//!
//! This crate provides struct `Xml` that can be used to extract typed information from request's body.
//!
//! Under the hood, [quick-xml](https://github.com/tafia/quick-xml) is used to parse payloads.
//!
//! ## Features
//!
//! - `encoding`: support non utf-8 payload
#![allow(clippy::module_name_repetitions)]

use std::ops::{Deref, DerefMut};

use axum_core::extract::FromRequest;
use axum_core::response::{IntoResponse, Response};
use bytes::Bytes;
use http::{header, HeaderValue, Request, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::rejection::XmlRejection;

mod rejection;
#[cfg(test)]
mod tests;

/// XML Extractor / Response.
///
/// When used as an extractor, it can deserialize request bodies into some type that
/// implements [`serde::Deserialize`]. If the request body cannot be parsed, or it does not contain
/// the `Content-Type: application/xml` header, it will reject the request and return a
/// `400 Bad Request` response.
///
/// # Extractor example
///
/// ```rust,no_run
/// use axum::{
///     extract,
///     routing::post,
///     Router,
/// };
/// use serde::Deserialize;
/// use axum_xml_extract::Xml;
///
/// #[derive(Deserialize)]
/// struct CreateUser {
///     email: String,
///     password: String,
/// }
///
/// async fn create_user(Xml(payload): Xml<CreateUser>) {
///     // payload is a `CreateUser`
/// }
///
/// let app = Router::new().route("/users", post(create_user));
/// # async {
/// # let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
/// # axum::serve(listener, app).await.unwrap();
/// # };
/// ```
///
/// When used as a response, it can serialize any type that implements [`serde::Serialize`] to
/// `XML`, and will automatically set `Content-Type: application/xml` header.
///
/// # Response example
///
/// ```
/// use axum::{
///     extract::Path,
///     routing::get,
///     Router,
/// };
/// use serde::Serialize;
/// use uuid::Uuid;
/// use axum_xml_extract::Xml;
///
/// #[derive(Serialize)]
/// struct User {
///     id: Uuid,
///     username: String,
/// }
///
/// async fn get_user(Path(user_id) : Path<Uuid>) -> Xml<User> {
///     let user = find_user(user_id).await;
///     Xml(user)
/// }
///
/// async fn find_user(user_id: Uuid) -> User {
///     // ...
///     # unimplemented!()
/// }
///
/// let app = Router::new().route("/users/{id}", get(get_user));
/// # async {
/// # let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
/// # axum::serve(listener, app).await.unwrap();
/// # };
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct Xml<T>(pub T);

impl<T, S> FromRequest<S> for Xml<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = XmlRejection;

    async fn from_request(
        req: Request<axum_core::body::Body>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        if xml_content_type(&req) {
            let bytes = Bytes::from_request(req, state).await?;

            let value = quick_xml::de::from_reader(&*bytes)?;

            Ok(Self(value))
        } else {
            Err(XmlRejection::MissingXMLContentType)
        }
    }
}

fn xml_content_type(req: &Request<axum_core::body::Body>) -> bool {
    let Some(content_type) = req.headers().get(header::CONTENT_TYPE) else {
        return false;
    };

    let Ok(content_type) = content_type.to_str() else {
        return false;
    };

    let Ok(mime) = content_type.parse::<mime::Mime>() else {
        return false;
    };

    let is_xml_content_type = (mime.type_() == "application" || mime.type_() == "text")
        && (mime.subtype() == "xml" || mime.suffix().is_some_and(|name| name == "xml"));

    is_xml_content_type
}

impl<T> Deref for Xml<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Xml<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> From<T> for Xml<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

impl<T> IntoResponse for Xml<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        match quick_xml::se::to_string(&self.0) {
            Ok(xml_string) => (
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/xml"),
                )],
                xml_string,
            )
                .into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
                )],
                err.to_string(),
            )
                .into_response(),
        }
    }
}
