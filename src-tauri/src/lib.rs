mod api;
mod dates;
mod db;
mod error;
mod money;
mod services;
mod state;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let state = state::AppState::initialize(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            api::health,
            api::bootstrap,
            api::settings_map,
            api::update_setting,
            api::list_audit_events,
            api::add_exchange_rate,
            api::list_exchange_rates,
            api::list_integration_status,
            api::list_accounts,
            api::create_account,
            api::update_account,
            api::archive_account,
            api::account_register,
            api::list_categories_cmd,
            api::create_category,
            api::update_category,
            api::archive_category,
            api::list_tags,
            api::list_income_sources,
            api::save_income_source,
            api::list_brokers,
            api::save_broker,
            api::list_transactions,
            api::get_transaction,
            api::create_transaction,
            api::update_transaction,
            api::set_transaction_status,
            api::reverse_transaction,
            api::delete_transaction,
            api::check_duplicate,
            api::list_budgets,
            api::save_budget,
            api::set_budget_active,
            api::delete_budget,
            api::list_goals,
            api::save_goal,
            api::add_goal_contribution,
            api::delete_goal_contribution,
            api::set_goal_status,
            api::delete_goal,
            api::list_instruments,
            api::portfolio_summary,
            api::save_instrument,
            api::delete_instrument,
            api::list_trades,
            api::save_trade,
            api::delete_trade,
            api::list_valuations,
            api::save_valuation,
            api::delete_valuation,
            api::list_liabilities,
            api::liabilities_summary,
            api::save_liability,
            api::save_liability_payment,
            api::delete_liability_payment,
            api::set_liability_status,
            api::delete_liability,
            api::net_worth_overview,
            api::capture_net_worth_snapshot,
            api::delete_net_worth_snapshot,
            api::analytics_report,
            api::income_report,
            api::month_calendar,
            api::list_journal_entries,
            api::save_journal_entry,
            api::delete_journal_entry,
            api::financial_health,
            api::fi_calculate,
            api::lab_compound,
            api::lab_loan,
            api::lab_inflation,
            api::import_preview,
            api::import_commit,
            api::list_import_jobs,
            api::cancel_import,
            api::create_backup,
            api::list_backups,
            api::delete_backup,
            api::validate_backup,
            api::create_encrypted_export,
            api::verify_encrypted_export,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rawjan");
}
