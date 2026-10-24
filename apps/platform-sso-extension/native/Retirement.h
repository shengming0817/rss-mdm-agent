// Two native metadata scopes only; no system registration state is inferred here.
#import <Foundation/Foundation.h>
static NSArray<NSString *> *RSSRegistrationScopes(void) { return @[@"registration",@"deviceRegistration"]; }
static NSDictionary<NSString *,NSString *> *RSSPendingRetirements(NSUserDefaults *defaults) {
    NSDictionary *stored=[defaults dictionaryForKey:@"retirements"];
    NSMutableDictionary *pending=[NSMutableDictionary dictionary];
    for(NSString *scope in RSSRegistrationScopes())if([stored[scope] isKindOfClass:NSString.class])pending[scope]=stored[scope];
    return pending;
}
static void RSSQueueRetirements(NSUserDefaults *defaults) {
    NSMutableDictionary *pending=[RSSPendingRetirements(defaults) mutableCopy];
    for(NSString *scope in RSSRegistrationScopes()) {
        NSString *metadata=[defaults stringForKey:scope];
        if(metadata && !pending[scope])pending[scope]=metadata;
    }
    if(pending.count)[defaults setObject:pending forKey:@"retirements"];
}
static void RSSCompleteRetirement(NSUserDefaults *defaults,NSString *scope,NSString *metadata) {
    NSMutableDictionary *pending=[RSSPendingRetirements(defaults) mutableCopy];
    if(![pending[scope] isEqual:metadata])return;
    [pending removeObjectForKey:scope];
    if([[defaults stringForKey:scope] isEqual:metadata])[defaults removeObjectForKey:scope];
    if(pending.count)[defaults setObject:pending forKey:@"retirements"];
    else[defaults removeObjectForKey:@"retirements"];
}
