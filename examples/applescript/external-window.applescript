-- UNRELEASED direct-startup contract; not the v0.5.1 behavior.
-- The external launcher replaces __RUN_COMMAND__ before running this template.
-- For a local test, replace __RUN_COMMAND__ with pwd.
-- Enable the default local profile in Conn > Settings > Automation first.
-- /bin/sh replaces the profile program on a private PTY; it is not typed at a prompt.
-- Conn adds no agent approval, policy or Grace step and records no private activity.
-- The child can echo/log its input; those are separate from Conn's activity history.
-- After Enter, this child exits and its finished terminal view remains open.
tell application "Conn"
    create window with default profile command "/bin/sh -c '__RUN_COMMAND__ ; echo \"Press [Enter] key to exit.\"; read ANSWER;'"
end tell
