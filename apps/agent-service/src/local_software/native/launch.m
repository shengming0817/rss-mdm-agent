// ref: macOS SDK AppKit/NSWorkspace.h openApplicationAtURL:configuration:completionHandler:.
#import <AppKit/AppKit.h>

int rss_open_local_application(const char *path) {
    @autoreleasepool {
        NSString *name = [[NSString alloc] initWithUTF8String:path];
        if (!name) return 1;
        NSURL *url = [NSURL fileURLWithPath:name isDirectory:YES];
        NSWorkspaceOpenConfiguration *configuration = [NSWorkspaceOpenConfiguration configuration];
        configuration.promptsUserIfNeeded = NO;
        __block BOOL completed = NO;
        __block int result = 1;
        [[NSWorkspace sharedWorkspace] openApplicationAtURL:url configuration:configuration
            completionHandler:^(NSRunningApplication *app, NSError *error) {
                result = app && !error ? 0 : 1;
                completed = YES;
            }];
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:15];
        while (!completed && [deadline timeIntervalSinceNow] > 0) {
            [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]];
        }
        return completed ? result : 1;
    }
}
