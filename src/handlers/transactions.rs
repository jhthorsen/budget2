use crate::{AppState, models::*};
use askama::Template;
use axum::response::IntoResponse;
use axum::{Form, extract::State};

#[derive(Template)]
#[template(path = "transaction.html")]
struct TransactionTemplate {
    accounts: Vec<Account>,
    categories: Vec<Category>,
    transaction: Transaction,
}

pub async fn delete_transaction_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(transaction): Form<Transaction>,
) -> super::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let mut transaction = Transaction::load(&state.pool, &transaction).await?;

    Ok(axum::response::Redirect::to("/dashboard").into_response())
}

pub async fn edit_transaction_page(
    State(state): State<crate::AppState>,
    session: tower_sessions::Session,
) -> super::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;

    render_transaction_page(
        &state,
        &Transaction {
            user_id: user.id,
            ..Transaction::default()
        },
    )
    .await
}

async fn render_transaction_page(state: &AppState, transaction: &Transaction) -> super::HttpResult {
    let template = TransactionTemplate {
        accounts: Account::all(&state.pool).await?,
        categories: Category::all(&state.pool).await?,
        transaction: transaction.clone(),
    };

    Ok(axum::response::Html(template.render()?).into_response())
}

pub async fn save_transaction_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(transaction): Form<Transaction>,
) -> super::HttpResult {
    let _ = auth::get_current_user(&state.pool, &session).await?;

    let mut transaction = match transaction.id > 0 {
        false => transaction,
        true => Transaction::load(&state.pool, &transaction)
            .await?
            .unwrap_or(transaction),
    };

    if transaction.original_amount == 0.0 {
        transaction.original_amount = transaction.amount;
    }

    let transaction = transaction.save(&state.pool).await?;

    let template = TransactionTemplate {
        accounts: Account::all(&state.pool).await?,
        categories: Category::all(&state.pool).await?,
        transaction,
    };

    Ok(axum::response::Html(template.render()?).into_response())
}
