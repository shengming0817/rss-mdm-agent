// ref: Apple AuthenticationServices headers, SDK 26.4. System callbacks run on the main queue.
#import <AppKit/AppKit.h>
#import <AuthenticationServices/AuthenticationServices.h>
#import <Security/Security.h>
#import "bridge.h"

static NSError *failure(void) { return [NSError errorWithDomain:@"com.rss.mdmagent.psso" code:1 userInfo:nil]; }
#import "Signing.h"
#import "Retirement.h"
static NSString *field(RSSPSSOHandle *h,uint32_t key) {
    const char *value=rss_psso_field(h,key);return value ? [NSString stringWithUTF8String:value] : nil;
}
@interface RSSPlatformSSO : NSViewController <ASAuthorizationProviderExtensionRegistrationHandler, ASAuthorizationProviderExtensionAuthorizationRequestHandler, NSURLSessionDataDelegate>
@property(copy) void (^registrationCompletion)(ASAuthorizationProviderExtensionRegistrationResult);
@property(strong) ASAuthorizationProviderExtensionLoginManager *manager;
@property(strong) ASAuthorizationProviderExtensionAuthorizationRequest *authorization;
@property(strong) NSURLSession *session;
@property(strong) NSURLSessionDataTask *task;
@property(strong) NSMutableData *responseBody;
@property(strong) NSHTTPURLResponse *response;
@property NSUInteger generation;
@end
@implementation RSSPlatformSSO
- (ASAuthorizationProviderExtensionPlatformSSOProtocolVersion)protocolVersion { return ASAuthorizationProviderExtensionPlatformSSOProtocolVersion1_0; }
- (ASAuthorizationProviderExtensionSupportedGrantTypes)supportedGrantTypes { return ASAuthorizationProviderExtensionSupportedGrantTypesJWTBearer; }
- (NSArray<ASAuthorizationProviderExtensionSigningAlgorithm> *)supportedDeviceSigningAlgorithms { return @[ASAuthorizationProviderExtensionSigningAlgorithmES256]; }
- (NSArray<ASAuthorizationProviderExtensionSigningAlgorithm> *)supportedUserSecureEnclaveKeySigningAlgorithms { return @[ASAuthorizationProviderExtensionSigningAlgorithmES256]; }
- (NSArray<ASAuthorizationProviderExtensionEncryptionAlgorithm> *)supportedDeviceEncryptionAlgorithms { return @[ASAuthorizationProviderExtensionEncryptionAlgorithmECDHE_A256GCM]; }
- (void)loadView {
    NSTextField *label=[NSTextField labelWithString:@"请先在 RSS 设备访问中关联当前账号，再从系统设置完成注册。注册失败不影响本地密码登录。"];
    label.frame=NSMakeRect(20,20,420,80);self.view=[[NSView alloc]initWithFrame:NSMakeRect(0,0,460,120)];[self.view addSubview:label];
}
- (void)beginDeviceRegistrationUsingLoginManager:(ASAuthorizationProviderExtensionLoginManager *)manager options:(ASAuthorizationProviderExtensionRequestOptions)options completion:(void (^)(ASAuthorizationProviderExtensionRegistrationResult))completion {
    [self registerUsing:manager user:NO options:options completion:completion];
}
- (void)beginUserRegistrationUsingLoginManager:(ASAuthorizationProviderExtensionLoginManager *)manager userName:(NSString *)name authenticationMethod:(ASAuthorizationProviderExtensionAuthenticationMethod)method options:(ASAuthorizationProviderExtensionRequestOptions)options completion:(void (^)(ASAuthorizationProviderExtensionRegistrationResult))completion {
    (void)name;
    if (method!=ASAuthorizationProviderExtensionAuthenticationMethodUserSecureEnclaveKey) { completion(ASAuthorizationProviderExtensionRegistrationResultFailedNoRetry);return; }
    [self registerUsing:manager user:YES options:options completion:completion];
}
- (void)registerUsing:(ASAuthorizationProviderExtensionLoginManager *)manager user:(BOOL)user options:(ASAuthorizationProviderExtensionRequestOptions)options completion:(void (^)(ASAuthorizationProviderExtensionRegistrationResult))completion {
    if(RSSPendingRetirements(NSUserDefaults.standardUserDefaults).count) {
        [self retireUsing:manager completion:^(BOOL done){ (void)done; }];
        completion(ASAuthorizationProviderExtensionRegistrationResultFailed);return;
    }
    const NSUInteger supported=ASAuthorizationProviderExtensionRequestOptionsUserInteractionEnabled|ASAuthorizationProviderExtensionRequestOptionsRegistrationRepair|ASAuthorizationProviderExtensionRequestOptionsRegistrationSharedDeviceKeys|ASAuthorizationProviderExtensionRequestOptionsUserKeyInvalid;
    if (options & ~supported) { completion(ASAuthorizationProviderExtensionRegistrationResultFailedNoRetry);return; }
    if (self.registrationCompletion || self.authorization) { completion(ASAuthorizationProviderExtensionRegistrationResultFailed);return; }
    if ((options & ASAuthorizationProviderExtensionRequestOptionsUserKeyInvalid) && !user) { completion(ASAuthorizationProviderExtensionRegistrationResultFailedNoRetry);return; }
    if (user && (options & ASAuthorizationProviderExtensionRequestOptionsUserKeyInvalid)) [manager resetUserSecureEnclaveKey];
    self.manager=manager;self.registrationCompletion=completion;NSUInteger generation=++self.generation;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW,30*NSEC_PER_SEC),dispatch_get_main_queue(), ^{
        [self finishRegistration:generation result:ASAuthorizationProviderExtensionRegistrationResultFailed];
    });
    SecKeyRef signing=[manager copyKeyForKeyType:user ? ASAuthorizationProviderExtensionKeyTypeUserSecureEnclaveKey : ASAuthorizationProviderExtensionKeyTypeCurrentDeviceSigning];
    SecKeyRef encryption=[manager copyKeyForKeyType:ASAuthorizationProviderExtensionKeyTypeCurrentDeviceEncryption];
    NSData *signingPoint=publicPoint(signing),*encryptionPoint=publicPoint(encryption);
    if (encryption) CFRelease(encryption);
    if (!signingPoint || !encryptionPoint) { if(signing)CFRelease(signing);[self finishRegistration:generation result:ASAuthorizationProviderExtensionRegistrationResultFailed];return; }
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED,0), ^{
        int32_t beginStatus=-1;RSSPSSOHandle *handle=rss_psso_begin(user?1:0,signingPoint.bytes,encryptionPoint.bytes,&beginStatus);
        int result=handle?rss_psso_register(handle,signMessage,(void *)signing):-1;CFRelease(signing);
        dispatch_async(dispatch_get_main_queue(), ^{
            if (generation==self.generation && self.registrationCompletion) {
                NSError *error=nil;BOOL saved=NO;
                if (result==0) {
                    if (user) {
                        ASAuthorizationProviderExtensionUserLoginConfiguration *config=[[ASAuthorizationProviderExtensionUserLoginConfiguration alloc]initWithLoginUserName:field(handle,RSSLoginName)];
                        saved=[manager saveUserLoginConfiguration:config error:&error];
                        if(saved)[NSUserDefaults.standardUserDefaults setObject:field(handle,RSSMetadata) forKey:@"registration"];
                    } else {
                        ASAuthorizationProviderExtensionLoginConfiguration *config=[[ASAuthorizationProviderExtensionLoginConfiguration alloc]initWithClientID:field(handle,RSSClient) issuer:field(handle,RSSIssuer) tokenEndpointURL:[NSURL URLWithString:field(handle,RSSToken)] jwksEndpointURL:[NSURL URLWithString:field(handle,RSSJwks)] audience:field(handle,RSSAudience)];
                        config.nonceEndpointURL=[NSURL URLWithString:field(handle,RSSNonce)];config.nonceResponseKeypath=@"nonce";config.serverNonceClaimName=@"request_nonce";
                        config.additionalScopes=@"urn:apple:platformsso";
                        config.customLoginRequestValues=@[[NSURLQueryItem queryItemWithName:@"platform_sso_version" value:@"1.0"]];
                        [config setCustomLoginRequestHeaderClaims:@{@"typ":field(handle,RSSLoginType)} returningError:&error];
                        if(!error)[config setCustomAssertionRequestHeaderClaims:@{@"typ":field(handle,RSSAssertionType)} returningError:&error];
                        saved=!error && [manager saveLoginConfiguration:config error:&error];
                        if(saved)[NSUserDefaults.standardUserDefaults setObject:field(handle,RSSMetadata) forKey:@"deviceRegistration"];
                    }
                }
                ASAuthorizationProviderExtensionRegistrationResult outcome=saved?ASAuthorizationProviderExtensionRegistrationResultSuccess:ASAuthorizationProviderExtensionRegistrationResultFailed;
                if(beginStatus==1)outcome=ASAuthorizationProviderExtensionRegistrationResultUserInterfaceRequired;
                [self finishRegistration:generation result:outcome];
            }
            rss_psso_free(handle);
        });
    });
}
- (void)finishRegistration:(NSUInteger)generation result:(ASAuthorizationProviderExtensionRegistrationResult)result {
    if(generation!=self.generation || !self.registrationCompletion)return;
    void (^completion)(ASAuthorizationProviderExtensionRegistrationResult)=self.registrationCompletion;self.registrationCompletion=nil;completion(result);
}
- (void)registrationDidCancel { NSUInteger generation=self.generation;[self finishRegistration:generation result:ASAuthorizationProviderExtensionRegistrationResultFailed];self.generation++; }
- (void)registrationDidComplete {
    NSString *metadata=[NSUserDefaults.standardUserDefaults stringForKey:@"registration"];
    BOOL device=self.manager.isDeviceRegistered,user=self.manager.isUserRegistered;
    if(metadata)dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY,0), ^{rss_psso_observe(metadata.UTF8String,device,user);});
    NSString *deviceMetadata=[NSUserDefaults.standardUserDefaults stringForKey:@"deviceRegistration"];
    if(deviceMetadata)dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY,0), ^{rss_psso_observe(deviceMetadata.UTF8String,device,user);});
    self.manager=nil;
}
- (void)retireUsing:(ASAuthorizationProviderExtensionLoginManager *)manager completion:(void (^)(BOOL))completion {
    NSDictionary<NSString *,NSString *> *pending=RSSPendingRetirements(NSUserDefaults.standardUserDefaults);
    if(!pending.count){completion(YES);return;}
    if(!manager){completion(NO);return;}
    BOOL device=manager.isDeviceRegistered,user=manager.isUserRegistered;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY,0), ^{
        NSMutableArray<NSString *> *completed=[NSMutableArray array];
        for(NSString *scope in pending) {
            NSString *metadata=pending[scope];
            BOOL observed=rss_psso_observe(metadata.UTF8String,device,user)==0;
            BOOL retired=rss_psso_unregister(metadata.UTF8String)==0;
            BOOL nativeCleared=!user && (![scope isEqual:@"deviceRegistration"] || !device);
            if(observed && retired && nativeCleared)[completed addObject:scope];
        }
        dispatch_async(dispatch_get_main_queue(), ^{
            for(NSString *scope in completed)RSSCompleteRetirement(NSUserDefaults.standardUserDefaults,scope,pending[scope]);
            completion(RSSPendingRetirements(NSUserDefaults.standardUserDefaults).count==0);
        });
    });
}
- (void)beginAuthorizationWithRequest:(ASAuthorizationProviderExtensionAuthorizationRequest *)request {
    if ([request.requestedOperation isEqual:ASAuthorizationProviderAuthorizationOperationConfigurationRemoved] || RSSPendingRetirements(NSUserDefaults.standardUserDefaults).count) {
        if(self.registrationCompletion)[self registrationDidCancel];
        if(self.authorization)[self cancelAuthorizationWithRequest:self.authorization];
        RSSQueueRetirements(NSUserDefaults.standardUserDefaults);
        if(request.loginManager) {
            request.loginManager.ssoTokens=nil;
            [request.loginManager resetUserSecureEnclaveKey];
        }
        self.authorization=request;NSUInteger generation=++self.generation;
        [self retireUsing:request.loginManager completion:^(BOOL done){
            if(self.authorization!=request || self.generation!=generation)return;
            self.authorization=nil;
            if(done)[request complete];else[request completeWithError:failure()];
        }];
        return;
    }
    if([request.callerBundleIdentifier isEqual:NSBundle.mainBundle.bundleIdentifier]){[request doNotHandle];return;}
    ASAuthorizationProviderExtensionLoginManager *manager=request.loginManager;
    NSString *metadata=[NSUserDefaults.standardUserDefaults stringForKey:@"registration"];
    if(!manager || !metadata || ![request.url.path hasSuffix:@"/browser/complete"]){[request doNotHandle];return;}
    if(self.authorization || self.registrationCompletion || request.httpBody.length>65536){[request completeWithError:failure()];return;}
    NSString *body=[[NSString alloc]initWithData:request.httpBody encoding:NSUTF8StringEncoding];
    NSString *token=manager.ssoTokens[@"access_token"];
    if(![token isKindOfClass:NSString.class] || !body){[manager userNeedsReauthenticationWithCompletion:^(NSError *e){}];[request completeWithError:failure()];return;}
    SecKeyRef signing=[manager copyKeyForKeyType:ASAuthorizationProviderExtensionKeyTypeUserSecureEnclaveKey];
    if(!signing){[request completeWithError:failure()];return;}
    self.authorization=request;NSUInteger generation=++self.generation;
    NSString *issuer=manager.loginConfiguration.issuer;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED,0), ^{
        char *bytes=rss_psso_browser(metadata.UTF8String,issuer.UTF8String,request.url.absoluteString.UTF8String,body.UTF8String,token.UTF8String,signMessage,(void *)signing);CFRelease(signing);
        NSData *encoded=bytes?[NSData dataWithBytes:bytes length:strlen(bytes)]:nil;rss_psso_string_free(bytes);
        dispatch_async(dispatch_get_main_queue(), ^{
            if(generation!=self.generation || self.authorization!=request)return;
            if(!encoded){self.authorization=nil;[request completeWithError:failure()];return;}
            NSMutableURLRequest *out=[NSMutableURLRequest requestWithURL:request.url];out.HTTPMethod=@"POST";out.HTTPBody=encoded;out.timeoutInterval=10;
            [out setValue:@"application/json" forHTTPHeaderField:@"Content-Type"];
            for(NSString *header in request.httpHeaders)if([header caseInsensitiveCompare:@"Origin"]==NSOrderedSame || [header caseInsensitiveCompare:@"Cookie"]==NSOrderedSame)[out setValue:request.httpHeaders[header] forHTTPHeaderField:header];
            NSURLSessionConfiguration *configuration=NSURLSessionConfiguration.ephemeralSessionConfiguration;configuration.URLCache=nil;configuration.HTTPCookieStorage=nil;configuration.timeoutIntervalForResource=10;
            self.session=[NSURLSession sessionWithConfiguration:configuration delegate:self delegateQueue:NSOperationQueue.mainQueue];
            self.responseBody=[NSMutableData data];self.response=nil;
            self.task=[self.session dataTaskWithRequest:out];[self.task resume];
        });
    });
}
- (void)cancelAuthorizationWithRequest:(ASAuthorizationProviderExtensionAuthorizationRequest *)request {
    if(self.authorization!=request)return;self.generation++;self.authorization=nil;[self.task cancel];[self.session invalidateAndCancel];self.task=nil;self.session=nil;
}
- (void)URLSession:(NSURLSession *)session dataTask:(NSURLSessionDataTask *)task didReceiveResponse:(NSURLResponse *)response completionHandler:(void (^)(NSURLSessionResponseDisposition))completion {
    if(session!=self.session || task!=self.task || ![response isKindOfClass:NSHTTPURLResponse.class] || response.expectedContentLength>65536) { completion(NSURLSessionResponseCancel);return; }
    NSHTTPURLResponse *http=(NSHTTPURLResponse *)response;
    if(http.statusCode>=300 && http.statusCode<400){completion(NSURLSessionResponseCancel);return;}
    self.response=http;completion(NSURLSessionResponseAllow);
}
- (void)URLSession:(NSURLSession *)session dataTask:(NSURLSessionDataTask *)task didReceiveData:(NSData *)data {
    if(session!=self.session || task!=self.task)return;
    if(data.length>65536-self.responseBody.length){[task cancel];return;}
    [self.responseBody appendData:data];
}
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task didCompleteWithError:(NSError *)error {
    if(session!=self.session || task!=self.task || !self.authorization)return;
    ASAuthorizationProviderExtensionAuthorizationRequest *request=self.authorization;self.authorization=nil;
    NSData *body=self.responseBody;NSHTTPURLResponse *response=self.response;
    self.task=nil;self.session=nil;self.response=nil;self.responseBody=nil;[session finishTasksAndInvalidate];
    if(error || !response)[request completeWithError:failure()];else[request completeWithHTTPResponse:response httpBody:body];
}
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task willPerformHTTPRedirection:(NSHTTPURLResponse *)response newRequest:(NSURLRequest *)request completionHandler:(void (^)(NSURLRequest *))completion { (void)session;(void)task;(void)response;(void)request;completion(nil); }
@end
