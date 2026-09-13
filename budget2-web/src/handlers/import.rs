use crate::helpers::*;
use askama::Template;
use axum::extract::{Multipart, State};
use axum::response::Redirect;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;

fn csrf_before_file(verified: bool) -> Result<(), String> {
    verified
        .then_some(())
        .ok_or_else(|| "CSRF token must be submitted before the CSV file".to_string())
}

#[derive(Debug, Serialize, Deserialize)]
struct UploadSession {
    file_id: String,
    file_path: String,
}

#[derive(Template)]
#[template(path = "import_upload_form.html")]
struct UploadTemplate {
    csrf_token: String,
}

#[derive(Template)]
#[template(path = "import_map_columns.html")]
struct MapTemplate {
    file_id: String,
    headers: Vec<String>,
    date_column: String,
    description_column: String,
    income_column: String,
    expense_column: String,
    account_column: String,
    category_column: String,
    date_formats: Vec<model::DateFormatOption>,
    csrf_token: String,
}

#[derive(Template)]
#[template(path = "imported.html")]
struct ImportedTemplate {
    result: model::ImportResult,
}

pub async fn upload_form(
    State(state): State<AppState>,
    session: tower_sessions::Session,
) -> HttpResult {
    let Ok((_, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(Redirect::to("/auth/login").into_response());
    };
    if matches!(membership.role, model::Role::Member) {
        return Ok(Redirect::to("/dashboard").into_response());
    }
    let csrf_token = csrf_token(&session).await?;
    Ok(Html(UploadTemplate { csrf_token }.render()?).into_response())
}

pub async fn upload_then_map_columns(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    mut multipart: Multipart,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(Redirect::to("/auth/login").into_response());
    };
    if matches!(membership.role, model::Role::Member) {
        return Ok(Redirect::to("/dashboard").into_response());
    }
    let mut uploaded = None;
    let mut csrf_verified = false;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| format!("Unable to upload: {err}"))?
    {
        if field.name() == Some("csrf_token") {
            let csrf = field
                .text()
                .await
                .map_err(|err| format!("Unable to read CSRF token: {err}"))?;
            verify_csrf(&session, &csrf).await?;
            csrf_verified = true;
            continue;
        }
        if field.name() != Some("csv_file") {
            continue;
        }
        csrf_before_file(csrf_verified)?;
        let filename = field.file_name().unwrap_or_default().to_ascii_lowercase();
        if !filename.ends_with(".csv") {
            return Err("Please upload a .csv file".into());
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|err| format!("Unable to read file: {err}"))?;
        if bytes.len() > MAX_UPLOAD_BYTES {
            return Err("CSV files must be 10 MB or smaller".into());
        }

        let file_id = format!(
            "budget2-upload-{}-{:032x}.csv",
            user.id,
            rand::random::<u128>()
        );
        let path = std::env::temp_dir().join(&file_id);
        std::fs::write(&path, &bytes).map_err(|err| format!("Unable to save file: {err}"))?;
        let headers = match model::csv::read_csv_headers(&path) {
            Ok(headers) if !headers.is_empty() => headers,
            Ok(_) => {
                std::fs::remove_file(&path).ok();
                return Err("CSV must contain a header row".into());
            }
            Err(err) => {
                std::fs::remove_file(&path).ok();
                return Err(format!("Unable to read CSV: {err}").into());
            }
        };
        uploaded = Some((file_id, path, headers));
        break;
    }

    let Some((file_id, path, headers)) = uploaded else {
        return Err("No CSV file uploaded".into());
    };
    session
        .insert(
            "csv_upload",
            UploadSession {
                file_id: file_id.clone(),
                file_path: path.to_string_lossy().into_owned(),
            },
        )
        .await
        .map_err(|err| format!("Unable to save upload session: {err}"))?;

    let suggestions = model::csv::suggest_columns(&path)?;
    let csrf_token = csrf_token(&session).await?;
    Ok(Html(
        MapTemplate {
            file_id,
            headers,
            date_column: suggestions.date_column,
            description_column: suggestions.description_column,
            income_column: suggestions.income_column,
            expense_column: suggestions.expense_column,
            account_column: suggestions.account_column,
            category_column: suggestions.category_column,
            date_formats: suggestions.date_formats,
            csrf_token,
        }
        .render()?,
    )
    .into_response())
}

#[cfg(test)]
mod tests {
    use super::csrf_before_file;

    #[test]
    fn csrf_must_precede_csv_bytes() {
        assert!(csrf_before_file(true).is_ok());
        assert!(csrf_before_file(false).is_err());
    }
}

pub async fn import_uploaded(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(mapping): Form<CsrfForm<model::ColumnMapping>>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(Redirect::to("/auth/login").into_response());
    };
    if matches!(membership.role, model::Role::Member) {
        return Ok(Redirect::to("/dashboard").into_response());
    }
    verify_csrf(&session, &mapping.csrf_token).await?;
    let upload: UploadSession = session
        .get("csv_upload")
        .await
        .map_err(|err| format!("Unable to read upload session: {err}"))?
        .ok_or("No active CSV upload")?;
    if upload.file_id != mapping.value.file_id {
        return Err("CSV upload session mismatch".into());
    }

    let path = PathBuf::from(&upload.file_path);
    let result = model::csv::import_csv_file(
        &state.pool,
        &user,
        membership.household_id,
        &path,
        mapping.value,
    )
    .await;
    std::fs::remove_file(&path).ok();
    session.remove::<UploadSession>("csv_upload").await.ok();
    Ok(Html(ImportedTemplate { result: result? }.render()?).into_response())
}
