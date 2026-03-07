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
    missing_workspace_url => MissingWorkspaceUrl = 3,
    auth_rejected => AuthRejected = 4,
    request_failed => RequestFailed = 5,
    config_load_error => ConfigLoadError = 6,
    invalid_time_window => InvalidTimeWindow = 7,
    invalid_poll_interval => InvalidPollInterval = 8,
    channel_resolve_error => ChannelResolveError = 9,
    user_load_error => UserLoadError = 10,
}
