use rusty_foundationmodels::Session;
use crate::tools::build_os_tools;

const SYSTEM_INSTRUCTIONS: &str = "You are Rover, a macOS launcher and automation assistant. \
    Help the user launch apps, automate tasks, answer questions about their Mac, and run commands. \
    Be concise — respond in 1-2 sentences unless detail is needed. \
    When asked to perform an action (launch app, run script, open URL), use the appropriate tool directly.";

pub fn create_session() -> Option<Session> {
    if !rusty_foundationmodels::is_available() {
        return None;
    }
    Session::with_tools(SYSTEM_INSTRUCTIONS, build_os_tools()).ok()
}
