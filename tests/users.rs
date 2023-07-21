#[cfg(test)]
mod tests {
    use hellomouse_board_server::handlers::debug_handler::DebugHandler;
    use hellomouse_board_server::datahandler::DataHandler;
    use serde_json::Value;

    #[test]
    fn login() {
        let mut handler = DebugHandler::new();
        handler.init();
        assert_eq!(handler.login("admin", "password"), true, "login succeeds with valid username + pass");
        assert_eq!(handler.login("admin", "wrong_password"), false, "login fails with valid user, invalid pass");
        assert_eq!(handler.login("invalid_user", "password"), false, "login fails with invalid user");
    }

    #[test]
    fn get_user() {
        let mut handler = DebugHandler::new();
        handler.init();

        assert!(handler.get_user("admin").is_ok(), "Get an existing user");
        assert_eq!(handler.get_user("admin").unwrap().id, "admin", "Get an existing user, id matches");
        assert!(handler.get_user("not_real_user").is_err(), "Get a non-existing user");
    }

    #[test]
    fn delete_account() {
        let mut handler = DebugHandler::new();
        handler.init();
        assert!(handler.delete_account("admin").is_ok(), "Delete existing user");
        assert!(handler.delete_account("admin").is_err(), "Delete non-existing user");
    }

    #[test]
    fn change_account_settings() {
        let mut handler = DebugHandler::new();
        handler.init();

        let settings_change_1: Value = serde_json::from_str("{\"test\": 100, \"newkey\": 1}").unwrap();
        let settings_change_2: Value = serde_json::from_str("{\"newkey\": 200}").unwrap();

        assert!(handler.change_account_settings("admin", settings_change_1.clone()).is_ok(), "Change settings on existing user");
        assert_eq!(handler.get_user("admin").unwrap().settings.to_string(), "{\"newkey\":1,\"test\":100}", "Settings change 1 applied");
        assert!(handler.change_account_settings("admin", settings_change_2.clone()).is_ok(), "Change settings on existing user");
        assert_eq!(handler.get_user("admin").unwrap().settings.to_string(), "{\"newkey\":200,\"test\":100}", "Settings change 1 applied");

        assert!(handler.change_account_settings("fake", settings_change_2.clone()).is_err(), "Change settings on non-existing user");
    }

    #[test]
    fn boards() {
        let mut handler = DebugHandler::new();
        handler.init();

        assert!(handler.create_board(
                "MyBoard".to_string(),
                "admin",
                "A random description".to_string(),
                "#FF0000".to_string(),
                Vec::new()).is_ok(),
            "Board 1 successfully inserted");
        assert!(handler.create_board(
                "MyBoard".to_string(),
                "admin",
                "A different board".to_string(),
                "#FFFF00".to_string(),
                Vec::new()).is_ok(),
            "Board 2 successfully inserted");

        let board = handler.get_boards("admin", None, None, None, None, &None, &None).unwrap()[0];
        assert_eq!(board.name, "MyBoard", "Board successfuly found after insert");

        let uuid = board.id.clone();
        assert!(handler.delete_board(&uuid).is_ok(), "Board deleted successfully");
        assert!(handler.delete_board(&uuid).is_err(), "Deleting already deleted board");

        let board = handler.get_boards("admin", None, None, None, None, &None, &None).unwrap()[0];
        assert!(handler.modify_board(&board.id.clone(), Some("Updated Name".to_string()), None, Some("#FFFFFF".to_string()), None).is_ok(), "Updated board info");
        let board = handler.get_boards("admin", None, None, None, None, &None, &None).unwrap()[0];
        assert_eq!(board.name, "Updated Name", "Board successfuly updated name");
        assert_eq!(board.color, "#FFFFFF", "Board successfuly updated color");
        assert_eq!(board.creator, "admin", "Creator not changed");
    }

    #[test]
    fn pins() {

    }
}
