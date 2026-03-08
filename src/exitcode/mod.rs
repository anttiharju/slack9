macro_rules! define_exitcodes {
    ( $( $fn:ident => $variant:ident = $code:expr ),* $(,)? ) => {
        #[repr(i32)]
        pub enum ExitCode {
            $( $variant = $code, )*
        }

        impl ExitCode {
            pub fn code(self) -> i32 { self as i32 }
        }

        $(
            pub fn $fn() -> i32 { ExitCode::$variant.code() }
        )*
    }
}

define_exitcodes! {
    missing_xoxd => MissingXoxd = 1,
    missing_xoxc => MissingXoxc = 2,
    missing_workspace => MissingWorkspace = 3,
    auth_rejected => AuthRejected = 4,
    request_failed => RequestFailed = 5,
    invalid_past => InvalidPast = 6,
    invalid_poll => InvalidPoll = 7,
    user_load_error => UserLoadError = 9,
    missing_team_id => MissingTeamId = 10,
    missing_user_id => MissingUserId = 11,
}
