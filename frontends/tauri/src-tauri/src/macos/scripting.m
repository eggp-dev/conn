#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>
#import <libproc.h>
#import <sys/proc_info.h>

extern char *conn_automation_call(const char *payload);
extern void conn_automation_free(char *payload);

@interface ConnAutomationCommand : NSScriptCommand
@end
@implementation ConnAutomationCommand
- (id)performDefaultImplementation {
    NSDictionary *arguments = self.evaluatedArguments ?: @{};
    NSMutableDictionary *params = [arguments mutableCopy];
    NSString *operation;
    switch (self.commandDescription.appleEventCode) {
        case 'opns': operation = @"session.create"; break;
        case 'wrte': operation = @"session.write"; params[@"text"] = self.directParameter ?: @""; break;
        case 'rqss':
        case 'rqst': operation = @"request.status"; params[@"requestId"] = self.directParameter ?: @""; break;
        case 'cncl': operation = @"request.cancel"; params[@"requestId"] = self.directParameter ?: @""; break;
        case 'rels': operation = @"session.release"; params[@"session"] = self.directParameter ?: @""; break;
        case 'ssta': operation = @"session.status"; params[@"session"] = self.directParameter ?: @""; break;
        default: self.scriptErrorNumber = -1708; return nil;
    }
    // Identity comes from the Apple Event, not caller-controlled script arguments.
    pid_t pid = [[self.appleEvent attributeDescriptorForKeyword:keySenderPIDAttr] int32Value];
    struct proc_bsdinfo info = {0};
    int infoSize = proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, sizeof(info));
    if (pid <= 0 || infoSize != sizeof(info)) {
        self.scriptErrorNumber = -1743;
        self.scriptErrorString = @"Could not identify the Apple Event sender";
        return nil;
    }
    NSString *identity = [NSString stringWithFormat:@"%d:%llu:%llu", pid,
        (unsigned long long)info.pbi_start_tvsec, (unsigned long long)info.pbi_start_tvusec];
    NSRunningApplication *sender = [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
    NSString *name = sender.localizedName ?: sender.bundleIdentifier ?: [NSString stringWithUTF8String:info.pbi_name];
    if (!name.length) name = @"AppleScript caller";
    BOOL stateOnly = self.commandDescription.appleEventCode == 'rqss';
    NSDictionary *request = @{@"operation": operation, @"params": params, @"identity": identity, @"name": name};
    NSError *error = nil;
    NSData *data = [NSJSONSerialization dataWithJSONObject:request options:0 error:&error];
    if (!data) { self.scriptErrorNumber = -1700; self.scriptErrorString = error.localizedDescription; return nil; }
    NSString *payload = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    [self suspendExecution];
    // Suspending returns first. Shell creation never blocks the Cocoa event loop.
    dispatch_async(dispatch_get_main_queue(), ^{
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            char *raw = conn_automation_call(payload.UTF8String);
            NSString *reply = raw ? [NSString stringWithUTF8String:raw] : nil;
            if (raw) conn_automation_free(raw);
            NSDictionary *result = reply ? [NSJSONSerialization JSONObjectWithData:[reply dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil] : nil;
            dispatch_async(dispatch_get_main_queue(), ^{
                id value = result[@"result"];
                if (!result || result[@"error"]) {
                    self.scriptErrorNumber = -2700;
                    self.scriptErrorString = result[@"error"] ?: @"Automation adapter unavailable";
                    value = nil;
                } else if (stateOnly) {
                    value = value[@"state"];
                } else if ([value isKindOfClass:[NSDictionary class]] || [value isKindOfClass:[NSArray class]]) {
                    // Status is portable JSON; create/write return plain string IDs.
                    NSData *encoded = [NSJSONSerialization dataWithJSONObject:value options:NSJSONWritingSortedKeys error:nil];
                    value = [[NSString alloc] initWithData:encoded encoding:NSUTF8StringEncoding];
                }
                [self resumeExecutionWithResult:value];
            });
        });
    });
    return nil;
}
@end

void conn_script_link(void) {
    // Keep the class in the static archive and initialize Cocoa's bundle dictionary.
    [ConnAutomationCommand class];
    [NSScriptSuiteRegistry sharedScriptSuiteRegistry];
}
