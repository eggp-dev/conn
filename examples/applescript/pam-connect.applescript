-- Run the entire script in ONE osascript process. Session ownership follows the
-- Apple Event sender; another osascript process cannot reuse these IDs.
-- First enable a profile in Conn > Settings > Automation.
-- Usage: osascript pam-connect.applescript "ssh user@host"
-- With no argument, the harmless demo command is pwd.
-- Never put passwords or tokens in the command. Use SSH-managed authentication.
on run argv
    set connectionCommand to "pwd"
    if (count of argv) > 0 then set connectionCommand to item 1 of argv
    tell application "Conn"
        activate
        set sessionID to create session
        set requestID to write text connectionCommand to session sessionID intent "Open a connection requested by the external launcher"
        repeat 1200 times
            set currentState to request state requestID
            if currentState is "delivered" then
                release session sessionID
                return "Input delivered. The terminal is yours; this does not confirm command or login completion."
            end if
            if currentState is in {"denied", "cancelled", "failed"} then
                set details to request status requestID
                error details
            end if
            delay 0.1
        end repeat
        cancel request requestID
        error "Timed out. Inspect Conn before retrying; delivered input cannot be undone."
    end tell
end run
