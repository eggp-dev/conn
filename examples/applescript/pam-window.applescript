-- PAM replaces __RUN_COMMAND__ before running this template.
-- For a Script Editor test, replace __RUN_COMMAND__ with pwd.
-- Enable the default profile in Conn > Settings > Automation first.
-- Conn asks for control and applies command policy before delivering the command.
tell application "Conn"
    create window with default profile command "/bin/sh -c '__RUN_COMMAND__ ; echo \"Press [Enter] key to exit.\"; read ANSWER;'"
end tell
