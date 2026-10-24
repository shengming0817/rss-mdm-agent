// Public metadata only. Each exact registration retains its own retirement intent.
#import <Foundation/Foundation.h>
static NSArray<NSString *> *RSSRegistrationScopes(void) { return @[@"registration",@"deviceRegistration"]; }
static NSDictionary<NSString *,NSString *> *RSSPendingRetirements(NSUserDefaults *defaults) {
    return [defaults dictionaryForKey:@"retirements"] ?: @{};
}
static void RSSQueueInflight(NSUserDefaults *defaults) {
    NSMutableDictionary *pending=[RSSPendingRetirements(defaults) mutableCopy];
    NSDictionary *inflight=[defaults dictionaryForKey:@"inflight"];
    for(NSString *scope in RSSRegistrationScopes())if(inflight[scope])pending[inflight[scope]]=scope;
    if(pending.count)[defaults setObject:pending forKey:@"retirements"];
}
static void RSSQueueRetirements(NSUserDefaults *defaults) {
    NSMutableDictionary *pending=[RSSPendingRetirements(defaults) mutableCopy];
    NSDictionary *inflight=[defaults dictionaryForKey:@"inflight"];
    for(NSString *scope in RSSRegistrationScopes()) {
        NSString *metadata=[defaults stringForKey:scope];
        if(metadata)pending[metadata]=scope;
        metadata=inflight[scope];
        if(metadata)pending[metadata]=scope;
    }
    if(pending.count)[defaults setObject:pending forKey:@"retirements"];
}
static BOOL RSSSetInflight(NSUserDefaults *defaults,NSString *scope,NSString *metadata) {
    NSMutableDictionary *inflight=[[defaults dictionaryForKey:@"inflight"] mutableCopy] ?: [NSMutableDictionary dictionary];
    if(metadata)inflight[scope]=metadata;else[inflight removeObjectForKey:scope];
    if(inflight.count)[defaults setObject:inflight forKey:@"inflight"];else[defaults removeObjectForKey:@"inflight"];
    return [defaults synchronize]; // Fence metadata before issuing a remote mutation.
}
// This records a save intent, never an assertion that Apple accepted the configuration.
static BOOL RSSBeginSave(NSUserDefaults *defaults,NSString *scope,NSString *metadata) {
    [defaults setObject:metadata forKey:scope];
    return [defaults synchronize];
}
static void RSSCompleteRetirement(NSUserDefaults *defaults,NSString *scope,NSString *metadata) {
    NSMutableDictionary *pending=[RSSPendingRetirements(defaults) mutableCopy];
    if(![pending[metadata] isEqual:scope])return;
    [pending removeObjectForKey:metadata];
    if([[defaults stringForKey:scope] isEqual:metadata])[defaults removeObjectForKey:scope];
    if([[[defaults dictionaryForKey:@"inflight"] objectForKey:scope] isEqual:metadata])RSSSetInflight(defaults,scope,nil);
    if(pending.count)[defaults setObject:pending forKey:@"retirements"];
    else[defaults removeObjectForKey:@"retirements"];
}
