-- UNRELEASED private external-input contract; not the v0.5.1 behavior.
-- Run the entire script in ONE osascript process. Ownership follows the native
-- sender process; another invocation cannot reuse these session handles.
-- Enable the default profile in Conn > Settings > Automation first.
-- Usage: osascript external-connect.applescript "pwd"
-- Input is delivered privately, without agent approval or activity recording.
-- Terminal echo, child logs, argv and shell history remain separate surfaces.
on run argv
    set inputLine to "pwd"
    if (count of argv) > 0 then set inputLine to item 1 of argv
    tell application "Conn"
        activate
        set sessionID to create session
        set requestID to write text inputLine to session sessionID
        repeat 1200 times
            set currentState to request state requestID
            if currentState is "delivered" then
                release session sessionID
                return "Input delivered. The private terminal remains open; command completion is not confirmed."
            end if
            if currentState is in {"cancelled", "failed"} then
                error "External input stopped. Inspect the terminal before creating another session."
            end if
            delay 0.1
        end repeat
        cancel request requestID
        error "Timed out. Inspect Conn before retrying; delivered input cannot be undone."
    end tell
end run
