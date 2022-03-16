/// Services are external dependencies to run by the application
///
/// They are defined in the .toml configuration for safety reason
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Service {
    /// A service can be a program to run in a subprocess
    #[serde(rename = "shell")]
    UnixShell {
        /// string alias to call the service in the .vsl files
        name: String,
        #[serde(with = "humantime_serde")]
        /// a duration after which the subprocess will be forced-kill
        timeout: std::time::Duration,
        /// optional: a user to run the subprocess under
        #[serde(default)]
        user: Option<String>,
        /// optional: a group to run the subprocess under
        #[serde(default)]
        group: Option<String>,
        /// the command to execute in the subprocess
        command: String,
        /// optional: parameters directly given to the executed program (argc, argv)
        args: Option<String>,
    },
}
