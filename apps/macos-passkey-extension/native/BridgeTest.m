// ABI-only probe: no system Provider activation, no Vault creation, no user authentication.
#import <Foundation/Foundation.h>
#import <AuthenticationServices/AuthenticationServices.h>
#import "Bridge.h"
#include <assert.h>
int main(void) { @autoreleasepool {
    NSMutableArray *hash=[NSMutableArray new];for(int i=0;i<32;i++)[hash addObject:@7];
    NSDictionary *request=@{@"kind":@"register",@"rp":@"example.com",@"hash":hash,@"user_handle":@[@1],@"username":@"test",@"algorithms":@[@(-7)],@"exclude":@[]};
    NSData *encoded=[NSJSONSerialization dataWithJSONObject:request options:0 error:NULL];
    NSString *json=[[NSString alloc]initWithData:encoded encoding:NSUTF8StringEncoding];
    assert(rss_passkey_validate(json.UTF8String)==0);
    assert(rss_passkey_validate("{\"kind\":\"register\"}")!=0);
    NSData *idValue=[NSData dataWithBytes:"id" length:2];
    ASPasskeyCredentialIdentity *identity=[[ASPasskeyCredentialIdentity alloc]initWithRelyingPartyIdentifier:@"example.com" userName:@"test" credentialID:idValue userHandle:idValue recordIdentifier:@"id"];
    ASPasskeyCredentialRequest *native=[[ASPasskeyCredentialRequest alloc]initWithCredentialIdentity:identity clientDataHash:[NSData dataWithLength:32] userVerificationPreference:ASAuthorizationPublicKeyCredentialUserVerificationPreferenceRequired supportedAlgorithms:@[@(-7)]];
    assert(native.clientDataHash.length==32);
    ASPasskeyRegistrationCredential *registration=[[ASPasskeyRegistrationCredential alloc]initWithRelyingParty:@"example.com" clientDataHash:native.clientDataHash credentialID:idValue attestationObject:idValue];
    assert([registration.credentialID isEqual:idValue]);
    ASPasskeyAssertionCredential *assertion=[[ASPasskeyAssertionCredential alloc]initWithUserHandle:idValue relyingParty:@"example.com" signature:idValue clientDataHash:native.clientDataHash authenticatorData:idValue credentialID:idValue];
    assert([assertion.userHandle isEqual:idValue]);
    return 0;
} }
