// ref: Apple AuthenticationServices/ASCredentialIdentityStore.h (SDK 26.4).
#import <AuthenticationServices/AuthenticationServices.h>
#import <Foundation/Foundation.h>
#include <stdatomic.h>
static atomic_int enabled;
static atomic_bool reading;
static NSData *bytes(NSArray *values) {
    NSMutableData *result = [NSMutableData dataWithLength:values.count];
    unsigned char *out = result.mutableBytes;
    for (NSUInteger i=0;i<values.count;i++) out[i]=[values[i] unsignedCharValue];
    return result;
}
int rss_passkey_index_state(void) {
    if (!atomic_exchange(&reading, true)) {
        [ASCredentialIdentityStore.sharedStore getCredentialIdentityStoreStateWithCompletion:^(ASCredentialIdentityStoreState *state) {
            atomic_store(&enabled, state.enabled ? 1 : 2);
            atomic_store(&reading, false);
        }];
    }
    return atomic_load(&enabled);
}
void rss_passkey_index_replace(const char *json, void *context, void (*complete)(void *,int)) {
    @autoreleasepool {
        NSData *data = [[NSString stringWithUTF8String:json] dataUsingEncoding:NSUTF8StringEncoding];
        NSArray *entries = [NSJSONSerialization JSONObjectWithData:data options:0 error:NULL];
        if (![entries isKindOfClass:NSArray.class]) { complete(context,0); return; }
        NSMutableArray *identities = [NSMutableArray new];
        for (NSDictionary *entry in entries) {
            NSData *idValue = bytes(entry[@"credentialId"]);
            [identities addObject:[[ASPasskeyCredentialIdentity alloc] initWithRelyingPartyIdentifier:entry[@"rpId"]
                userName:entry[@"username"] credentialID:idValue userHandle:bytes(entry[@"userHandle"])
                recordIdentifier:[idValue base64EncodedStringWithOptions:0]]];
        }
        // A late callback cannot acknowledge a snapshot as timely. Until the callback arrives,
        // retain a single in-flight publication in this process; no retries accumulate.
        NSTimeInterval began = NSProcessInfo.processInfo.systemUptime;
        [ASCredentialIdentityStore.sharedStore replaceCredentialIdentityEntries:identities completion:^(BOOL success,NSError *error) {
            (void)error;
            complete(context, success && NSProcessInfo.processInfo.systemUptime - began < 5 ? 1 : 0);
        }];
    }
}
