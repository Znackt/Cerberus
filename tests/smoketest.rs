use cerberus::AppService;

#[test]
fn test_app_initialization_and_remotes() {
    let app_service = AppService::new(None);
    let _ = app_service.list_rclone_remotes();
}
