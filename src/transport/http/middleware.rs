//! Central request policy: safe correlation, CSRF, bounded limits, deadlines, and no-store.
use super::{
    AppState,
    error::{ApiError, REQUEST_ID},
};
use axum::{
    extract::{ConnectInfo, MatchedPath, Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Instant,
};
pub async fn common(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 64
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    REQUEST_ID.scope(id.clone(),async move{
  let started=Instant::now();let method=request.method().clone();
  let route=request.extensions().get::<MatchedPath>().map(|p|p.as_str()).unwrap_or("unmatched").to_owned();
  let ip=request.extensions().get::<ConnectInfo<SocketAddr>>().map(|i|i.0.ip()).unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
  let login=method==Method::POST&&request.uri().path()=="/api/v1/admin/sessions";
  let health=request.uri().path().starts_with("/health/");
  let policy=if !health&&method!=Method::OPTIONS&&!state.limiter.allow(ip,login){Err(ApiError::transport(StatusCode::TOO_MANY_REQUESTS,"RATE_LIMITED"))}
  else if request.headers().get(header::CONTENT_LENGTH).and_then(|h|h.to_str().ok()).and_then(|s|s.parse::<usize>().ok()).is_some_and(|n|n>state.settings.http.body_limit){Err(ApiError::transport(StatusCode::PAYLOAD_TOO_LARGE,"BODY_TOO_LARGE"))}
  else if request.uri().path().starts_with("/api/v1/")&&!matches!(method,Method::GET|Method::HEAD|Method::OPTIONS){state.csrf.validate(request.headers()).map(|_|()).map_err(Into::into)}else{Ok(())};
  let mut response=match policy{
   Err(error)=>error.into_response(),
   Ok(())=>match tokio::time::timeout(state.settings.http.request_timeout,next.run(request)).await{Ok(response)=>response,Err(_)=>ApiError::transport(StatusCode::GATEWAY_TIMEOUT,"REQUEST_TIMEOUT").into_response()},
  };
  response.headers_mut().insert(header::CACHE_CONTROL,HeaderValue::from_static("no-store"));
  if let Ok(value)=HeaderValue::from_str(&id){response.headers_mut().insert("x-request-id",value);}
  response.headers_mut().insert("x-content-type-options",HeaderValue::from_static("nosniff"));
  tracing::info!(request_id=%id,method=%method,route=%route,status=response.status().as_u16(),elapsed_ms=started.elapsed().as_millis(),"request completed");response
 }).await
}
