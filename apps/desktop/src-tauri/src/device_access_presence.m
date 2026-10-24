// ref: Apple LocalAuthentication/LAContext.h; AppKit/NSRunningApplication.h.
#import <AppKit/AppKit.h>
#import <LocalAuthentication/LocalAuthentication.h>
#include <unistd.h>
static BOOL foreground(void) {
    __block BOOL active=NO;
    dispatch_sync(dispatch_get_main_queue(), ^{ active=NSRunningApplication.currentApplication.active; });
    return active && geteuid()!=0;
}
int rss_device_access_presence(void) {
    @autoreleasepool {
        // Called only from the native Rust blocking pool; never block AppKit's main queue.
        if(!foreground())return 0;
        LAContext *context=[LAContext new];NSError *error=nil;
        if(![context canEvaluatePolicy:LAPolicyDeviceOwnerAuthentication error:&error])return 0;
        dispatch_semaphore_t done=dispatch_semaphore_create(0);__block BOOL allowed=NO;
        [context evaluatePolicy:LAPolicyDeviceOwnerAuthentication localizedReason:@"确认当前本地账号的 RSS 设备访问设置" reply:^(BOOL success,NSError *error){
            (void)error;allowed=success;dispatch_semaphore_signal(done);
        }];
        BOOL finished=dispatch_semaphore_wait(done,dispatch_time(DISPATCH_TIME_NOW,30*NSEC_PER_SEC))==0;
        [context invalidate];return finished && allowed && foreground();
    }
}
