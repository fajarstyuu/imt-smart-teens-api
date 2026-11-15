//! One safe Indonesian error envelope, including extractor and middleware failures.
use crate::shared::application::AppError;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
tokio::task_local! {pub static REQUEST_ID:String;}
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    details: Option<serde_json::Value>,
}
impl ApiError {
    pub fn transport(status: StatusCode, code: &'static str) -> Self {
        Self {
            status,
            code,
            details: None,
            message: match status {
                StatusCode::BAD_REQUEST => "Permintaan tidak valid.",
                StatusCode::PAYLOAD_TOO_LARGE => "Data terlalu besar.",
                StatusCode::UNSUPPORTED_MEDIA_TYPE => "Gunakan format JSON.",
                StatusCode::METHOD_NOT_ALLOWED => "Metode tidak diizinkan.",
                StatusCode::NOT_FOUND => "Data tidak ditemukan.",
                StatusCode::TOO_MANY_REQUESTS => "Terlalu banyak permintaan. Coba lagi nanti.",
                StatusCode::GATEWAY_TIMEOUT => "Waktu permintaan habis. Coba lagi.",
                _ => "Permintaan tidak dapat diproses.",
            },
        }
    }
}
impl From<AppError> for ApiError {
    fn from(value: AppError) -> Self {
        let (status, code, message) = match value {
            AppError::ConflictDetails { code, details } => {
                return Self {
                    status: StatusCode::CONFLICT,
                    code,
                    details: Some(match details {
                        crate::shared::error::ConflictDetails::AnswerRevision {
                            snapshot_question_id,
                            option_ids,
                            revision,
                            saved_at,
                        } => {
                            json!({"latest_answer": {"snapshot_question_id":snapshot_question_id,"option_ids":option_ids,"revision":revision,"saved_at":saved_at}})
                        }
                    }),
                    message: "Data telah berubah. Muat ulang sebelum menyimpan.",
                };
            }
            AppError::Invalid(code) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                code,
                match code {
                    "QUESTION_VERSION_REQUIRED" => {
                        "Pilih versi pertanyaan untuk melihat jawaban historis."
                    }
                    _ => "Periksa kembali data yang dimasukkan.",
                },
            ),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Sesi tidak valid atau kredensial salah. Silakan masuk kembali.",
            ),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "Anda tidak memiliki izin untuk tindakan ini.",
            ),
            AppError::NotFound => (StatusCode::NOT_FOUND, "NOT_FOUND", "Data tidak ditemukan."),
            AppError::Conflict(code) => (
                StatusCode::CONFLICT,
                code,
                match code {
                    "USE_PATH_ORDER" => {
                        "Ubah posisi melalui pengaturan urutan materi, lalu muat ulang editor."
                    }
                    "CONTENT_REVISION_CONFLICT" => {
                        "Data konten telah berubah. Muat ulang sebelum menyimpan."
                    }
                    "ENROLLMENT_SELECTION_REQUIRED" => {
                        "Pilih satu pendaftaran untuk melihat hasil siswa."
                    }
                    "CONTENT_CONFIGURATION_REQUIRED" => {
                        "Konfigurasi konten belum tersedia. Hubungi pengelola."
                    }
                    "SCHOOL_NAME_EXISTS" => "Nama sekolah sudah terdaftar.",
                    "SCHOOL_ARCHIVED" => "Sekolah telah diarsipkan.",
                    "SCHOOL_REVISION_CONFLICT" => {
                        "Data telah berubah. Muat ulang sebelum menyimpan."
                    }
                    _ => "Tindakan bertentangan dengan kondisi data saat ini.",
                },
            ),
            AppError::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                "Layanan sementara tidak tersedia. Coba lagi.",
            ),
            AppError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Terjadi kesalahan. Coba lagi.",
            ),
        };
        Self {
            status,
            code,
            message,
            details: None,
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let request_id = REQUEST_ID
            .try_with(Clone::clone)
            .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
        let mut body =
            json!({"error":{"code":self.code,"message":self.message},"request_id":request_id});
        if let Some(details) = self.details {
            body["error"]["details"] = details;
        }
        (self.status, Json(body)).into_response()
    }
}
