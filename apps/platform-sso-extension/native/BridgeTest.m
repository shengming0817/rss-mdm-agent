// Test-only process: verifies the actual C/Rust ABI and Security signature conversion.
// Its ephemeral test key is never used by the shipped extension or a registration.
#import "bridge.h"
#import "Signing.h"
#import "Retirement.h"
#include <assert.h>
static NSString *encode(NSData *data) {
    return [[[data base64EncodedStringWithOptions:0] stringByReplacingOccurrencesOfString:@"+" withString:@"-"] stringByReplacingOccurrencesOfString:@"/" withString:@"_"];
}
static NSData *decode(NSString *value) {
    value=[[value stringByReplacingOccurrencesOfString:@"-" withString:@"+"] stringByReplacingOccurrencesOfString:@"_" withString:@"/"];
    while(value.length%4)value=[value stringByAppendingString:@"="];
    return [[NSData alloc]initWithBase64EncodedString:value options:0];
}
static NSString *json(id value) { return [[NSString alloc]initWithData:[NSJSONSerialization dataWithJSONObject:value options:0 error:NULL] encoding:NSUTF8StringEncoding]; }
static int signingCalls=0;
static int32_t testSign(void *context,const uint8_t *bytes,size_t length,uint8_t *output) {
    signingCalls++;return signMessage(context,bytes,length,output);
}
int main(void) { @autoreleasepool {
    // Modern ephemeral Security key, matching the system key API; never persist a test key.
    SecKeyRef key=SecKeyCreateRandomKey((__bridge CFDictionaryRef)@{(__bridge id)kSecUseDataProtectionKeychain:@YES,(__bridge id)kSecAttrIsPermanent:@NO,(__bridge id)kSecAttrKeyType:(__bridge id)kSecAttrKeyTypeECSECPrimeRandom,(__bridge id)kSecAttrKeySizeInBits:@256},NULL);
    assert(key);NSData *point=publicPoint(key);assert(point.length==65);
    NSString *x=[[encode([point subdataWithRange:NSMakeRange(1,32)]) componentsSeparatedByString:@"="]firstObject];
    NSString *y=[[encode([point subdataWithRange:NSMakeRange(33,32)]) componentsSeparatedByString:@"="]firstObject];
    NSDictionary *publicKey=@{@"kty":@"EC",@"crv":@"P-256",@"x":x,@"y":y};
    NSString *id1=@"00000000-0000-0000-0000-000000000001";
    NSString *issuer=[@"https://mdm.example/api/v1/apple/platform-sso/" stringByAppendingString:id1];
    NSString *target=[issuer stringByAppendingString:@"/browser/complete"];
    NSString *metadata=json(@{@"context":@{@"tenant":id1,@"agentRegistration":id1,@"installation":id1,@"origin":@"https://mdm.example",@"kind":@"user",@"binding":id1,@"bindingGeneration":@1,@"generatedUid":id1},@"sourceId":id1,@"sourceVersion":@1,@"clientId":id1,@"signingKey":publicKey,@"encryptionKey":publicKey});
    NSString *body=json(@{@"challengeId":id1,@"nonce":@"test-nonce",@"proof":@""});
    NSString *token=[@"a" stringByPaddingToLength:43 withString:@"a" startingAtIndex:0];
    char *result=rss_psso_browser(metadata.UTF8String,issuer.UTF8String,target.UTF8String,body.UTF8String,token.UTF8String,testSign,(void *)key);
    assert(result && signingCalls==1);NSDictionary *response=[NSJSONSerialization JSONObjectWithData:[[NSString stringWithUTF8String:result] dataUsingEncoding:NSUTF8StringEncoding] options:0 error:NULL];rss_psso_string_free(result);
    NSArray *parts=[response[@"proof"] componentsSeparatedByString:@"."];assert(parts.count==3);
    NSData *input=[[NSString stringWithFormat:@"%@.%@",parts[0],parts[1]]dataUsingEncoding:NSUTF8StringEncoding];
    SecKeyRef publicKeyRef=SecKeyCopyPublicKey(key);
    assert(SecKeyVerifySignature(publicKeyRef,kSecKeyAlgorithmECDSASignatureMessageRFC4754SHA256,(__bridge CFDataRef)input,(__bridge CFDataRef)decode(parts[2]),NULL));
    assert(!rss_psso_browser(metadata.UTF8String,issuer.UTF8String,"https://other.example/browser/complete",body.UTF8String,token.UTF8String,testSign,(void *)key));assert(signingCalls==1);
    int32_t status=0;assert(!rss_psso_begin(1,NULL,NULL,&status) && status==-1);
    NSString *suite=[@"com.rss.psso.test." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults=[[NSUserDefaults alloc]initWithSuiteName:suite];
    RSSQueueRetirements(defaults);assert(RSSPendingRetirements(defaults).count==0);
    [defaults setObject:@"device-v1" forKey:@"deviceRegistration"];RSSQueueRetirements(defaults);
    assert(RSSPendingRetirements(defaults).count==1); // Device-only removal remains recoverable.
    [defaults setObject:@"user-v1" forKey:@"registration"];RSSQueueRetirements(defaults);
    assert(RSSPendingRetirements(defaults).count==2);
    RSSCompleteRetirement(defaults,@"registration",@"user-v1");assert(RSSPendingRetirements(defaults).count==1);
    [defaults setObject:@"device-v2" forKey:@"deviceRegistration"];
    RSSCompleteRetirement(defaults,@"deviceRegistration",@"device-v1");
    assert([[defaults stringForKey:@"deviceRegistration"] isEqual:@"device-v2"]);
    assert(RSSPendingRetirements(defaults).count==0);[defaults removePersistentDomainForName:suite];
    CFRelease(publicKeyRef);CFRelease(key);puts("Platform SSO native ABI and signature test passed; no system registration exercised");
} return 0; }
