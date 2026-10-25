// ref: Apple LocalAuthentication/LAContext.h; AppKit/NSAlert.h, NSSecureTextField.h.
#import <AppKit/AppKit.h>
#import <LocalAuthentication/LocalAuthentication.h>
#include <string.h>
#include <unistd.h>
static LAContext *activeContext;
static BOOL passwordVisible;
static BOOL foreground(void) {
    __block BOOL active=NO;
    dispatch_sync(dispatch_get_main_queue(), ^{ active=NSRunningApplication.currentApplication.active; });
    return active && geteuid()!=0;
}
void rss_personal_vault_cancel(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        [activeContext invalidate];
        if(passwordVisible)[NSApp abortModal];
    });
}
int rss_personal_vault_verify(void) {
    @autoreleasepool {
        if(!foreground())return 0;
        LAContext *context=[LAContext new]; NSError *error=nil;
        if(![context canEvaluatePolicy:LAPolicyDeviceOwnerAuthentication error:&error])return 0;
        dispatch_sync(dispatch_get_main_queue(), ^{ activeContext=context; });
        dispatch_semaphore_t done=dispatch_semaphore_create(0); __block BOOL allowed=NO;
        [context evaluatePolicy:LAPolicyDeviceOwnerAuthentication localizedReason:@"解锁或管理此 OS 用户的个人 Passkey Vault" reply:^(BOOL success,NSError *error){
            (void)error; allowed=success; dispatch_semaphore_signal(done);
        }];
        BOOL finished=dispatch_semaphore_wait(done,dispatch_time(DISPATCH_TIME_NOW,120*NSEC_PER_SEC))==0;
        [context invalidate];
        dispatch_sync(dispatch_get_main_queue(), ^{ if(activeContext==context)activeContext=nil; });
        return finished && allowed && foreground();
    }
}
int rss_personal_vault_password(const char *label, unsigned char *output, size_t capacity) {
    @autoreleasepool {
        if(!foreground() || capacity<1025)return 0;
        __block int result=0;
        dispatch_sync(dispatch_get_main_queue(), ^{
            NSAlert *alert=[NSAlert new];
            alert.messageText=@"个人 Passkey Vault";
            alert.informativeText=[NSString stringWithUTF8String:label];
            [alert addButtonWithTitle:@"继续"]; [alert addButtonWithTitle:@"取消"];
            NSSecureTextField *field=[[NSSecureTextField alloc] initWithFrame:NSMakeRect(0,0,360,28)];
            alert.accessoryView=field; [alert.window setInitialFirstResponder:field];
            passwordVisible=YES;
            NSTimer *timer=[NSTimer timerWithTimeInterval:120 repeats:NO block:^(NSTimer *timer){(void)timer;[NSApp abortModal];}];
            [[NSRunLoop mainRunLoop] addTimer:timer forMode:NSModalPanelRunLoopMode];
            NSModalResponse response=[alert runModal];
            [timer invalidate]; passwordVisible=NO;
            if(response==NSAlertFirstButtonReturn && NSRunningApplication.currentApplication.active) {
                NSData *bytes=[field.stringValue dataUsingEncoding:NSUTF8StringEncoding];
                if(bytes.length>0 && bytes.length<=1024) { memcpy(output,bytes.bytes,bytes.length); result=(int)bytes.length; }
            }
            field.stringValue=@"";
        });
        return result;
    }
}
// These observers live for the application lifetime; callback context is an owned Rust Arc.
void rss_personal_vault_observe(void (*callback)(void *), void *context) {
    void (^changed)(NSNotification *)=^(NSNotification *note){(void)note;callback(context);rss_personal_vault_cancel();};
    NSNotificationCenter *workspace=NSWorkspace.sharedWorkspace.notificationCenter;
    for(NSString *name in @[NSWorkspaceSessionDidResignActiveNotification,NSWorkspaceWillSleepNotification,NSWorkspaceWillPowerOffNotification])
        [workspace addObserverForName:name object:nil queue:NSOperationQueue.mainQueue usingBlock:changed];
    [NSDistributedNotificationCenter.defaultCenter addObserverForName:@"com.apple.screenIsLocked" object:nil queue:NSOperationQueue.mainQueue usingBlock:changed];
}
