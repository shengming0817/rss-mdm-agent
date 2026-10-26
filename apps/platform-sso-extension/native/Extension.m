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
// Read only the system-provided configuration. Bootstrap/NSUserDefaults never supply these values.
static NSString *nativeConfiguration(ASAuthorizationProviderExtensionLoginManager *manager) {
    ASAuthorizationProviderExtensionLoginConfiguration *config=manager.loginConfiguration;
    if(!manager || !config)return @"null";
    NSMutableArray *values=[NSMutableArray array];
    for(NSURLQueryItem *item in config.customLoginRequestValues ?: @[]) [values addObject:@[item.name ?: @"",item.value ?: @""]];
    NSDictionary *actual=@{@"authentication":@((uint32_t)manager.authenticationMethod),@"clientId":config.clientID ?: @"",
        @"issuer":config.issuer ?: @"",@"audience":config.audience ?: @"",@"tokenEndpoint":config.tokenEndpointURL.absoluteString ?: @"",
        @"jwksEndpoint":config.jwksEndpointURL.absoluteString ?: @"",@"nonceEndpoint":config.nonceEndpointURL.absoluteString ?: @"",
        @"nonceResponseKeypath":config.nonceResponseKeypath ?: @"",@"serverNonceClaim":config.serverNonceClaimName ?: @"",
        @"additionalScopes":config.additionalScopes ?: @"",@"loginRequestValues":values,
        @"loginUserName":manager.userLoginConfiguration.loginUserName ?: NSNull.null};
    NSData *json=[NSJSONSerialization dataWithJSONObject:actual options:0 error:nil];
    return json && json.length<=16384 ? [[NSString alloc]initWithData:json encoding:NSUTF8StringEncoding] : @"null";
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
@property(copy) NSString *registrationScope;
@property(copy) NSString *registrationMetadata;
@end
@implementation RSSPlatformSSO
- (ASAuthorizationProviderExtensionPlatformSSOProtocolVersion)protocolVersion { return ASAuthorizationProviderExtensionPlatformSSOProtocolVersion1_0; }
- (ASAuthorizationProviderExtensionSupportedGrantTypes)supportedGrantTypes { return ASAuthorizationProviderExtensionSupportedGrantTypesJWTBearer | ASAuthorizationProviderExtensionSupportedGrantTypesPassword; }
- (NSArray<ASAuthorizationProviderExtensionSigningAlgorithm> *)supportedDeviceSigningAlgorithms { return @[ASAuthorizationProviderExtensionSigningAlgorithmES256]; }
- (NSArray<ASAuthorizationProviderExtensionSigningAlgorithm> *)supportedUserSecureEnclaveKeySigningAlgorithms { return @[ASAuthorizationProviderExtensionSigningAlgorithmES256]; }
- (NSArray<ASAuthorizationProviderExtensionEncryptionAlgorithm> *)supportedDeviceEncryptionAlgorithms { return @[ASAuthorizationProviderExtensionEncryptionAlgorithmECDHE_A256GCM]; }
- (void)loadView {
    NSTextField *label=[NSTextField labelWithString:@"请先在 RSS 设备访问中关联当前账号，再从系统设置完成注册。登录、解锁和恢复入口受组织策略约束；无法注册时请联系管理员。"];
    label.frame=NSMakeRect(20,20,420,80);self.view=[[NSView alloc]initWithFrame:NSMakeRect(0,0,460,120)];[self.view addSubview:label];
}
- (void)beginDeviceRegistrationUsingLoginManager:(ASAuthorizationProviderExtensionLoginManager *)manager options:(ASAuthorizationProviderExtensionRequestOptions)options completion:(void (^)(ASAuthorizationProviderExtensionRegistrationResult))completion {
    [self registerUsing:manager user:NO options:options completion:completion];
}
- (void)beginUserRegistrationUsingLoginManager:(ASAuthorizationProviderExtensionLoginManager *)manager userName:(NSString *)name authenticationMethod:(ASAuthorizationProviderExtensionAuthenticationMethod)method options:(ASAuthorizationProviderExtensionRequestOptions)options completion:(void (^)(ASAuthorizationProviderExtensionRegistrationResult))completion {
    (void)name;
    if (method!=manager.authenticationMethod || (method!=ASAuthorizationProviderExtensionAuthenticationMethodUserSecureEnclaveKey && method!=ASAuthorizationProviderExtensionAuthenticationMethodPassword)) { completion(ASAuthorizationProviderExtensionRegistrationResultFailedNoRetry);return; }
    [self registerUsing:manager user:YES options:options completion:completion];
}
- (void)registerUsing:(ASAuthorizationProviderExtensionLoginManager *)manager user:(BOOL)user options:(ASAuthorizationProviderExtensionRequestOptions)options completion:(void (^)(ASAuthorizationProviderExtensionRegistrationResult))completion {
    ASAuthorizationProviderExtensionAuthenticationMethod method=manager.authenticationMethod;
    if(method!=ASAuthorizationProviderExtensionAuthenticationMethodPassword && method!=ASAuthorizationProviderExtensionAuthenticationMethodUserSecureEnclaveKey){completion(ASAuthorizationProviderExtensionRegistrationResultFailedNoRetry);return;}
    if (self.registrationCompletion || self.authorization) { completion(ASAuthorizationProviderExtensionRegistrationResultFailed);return; }
    if([NSUserDefaults.standardUserDefaults dictionaryForKey:@"inflight"].count)RSSQueueInflight(NSUserDefaults.standardUserDefaults);
    if(RSSPendingRetirements(NSUserDefaults.standardUserDefaults).count) {
        [self retireUsing:manager completion:^(BOOL done){ (void)done; }];
        completion(ASAuthorizationProviderExtensionRegistrationResultFailed);return;
    }
    const NSUInteger supported=ASAuthorizationProviderExtensionRequestOptionsUserInteractionEnabled|ASAuthorizationProviderExtensionRequestOptionsRegistrationRepair|ASAuthorizationProviderExtensionRequestOptionsRegistrationSharedDeviceKeys|ASAuthorizationProviderExtensionRequestOptionsUserKeyInvalid;
    if (options & ~supported) { completion(ASAuthorizationProviderExtensionRegistrationResultFailedNoRetry);return; }
    if ((options & ASAuthorizationProviderExtensionRequestOptionsUserKeyInvalid) && !user) { completion(ASAuthorizationProviderExtensionRegistrationResultFailedNoRetry);return; }
    if (user && method==ASAuthorizationProviderExtensionAuthenticationMethodUserSecureEnclaveKey && (options & ASAuthorizationProviderExtensionRequestOptionsUserKeyInvalid)) [manager resetUserSecureEnclaveKey];
    self.registrationMetadata=nil;self.manager=manager;self.registrationScope=user?@"registration":@"deviceRegistration";self.registrationCompletion=completion;NSUInteger generation=++self.generation;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW,30*NSEC_PER_SEC),dispatch_get_main_queue(), ^{
        [self finishRegistration:generation result:ASAuthorizationProviderExtensionRegistrationResultFailed];
    });
    SecKeyRef signing=[manager copyKeyForKeyType:user && method==ASAuthorizationProviderExtensionAuthenticationMethodUserSecureEnclaveKey ? ASAuthorizationProviderExtensionKeyTypeUserSecureEnclaveKey : ASAuthorizationProviderExtensionKeyTypeCurrentDeviceSigning];
    SecKeyRef encryption=[manager copyKeyForKeyType:ASAuthorizationProviderExtensionKeyTypeCurrentDeviceEncryption];
    NSData *signingPoint=publicPoint(signing),*encryptionPoint=publicPoint(encryption);
    if (encryption) CFRelease(encryption);
    if (!signingPoint || !encryptionPoint) { if(signing)CFRelease(signing);[self finishRegistration:generation result:ASAuthorizationProviderExtensionRegistrationResultFailed];return; }
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED,0), ^{
        int32_t beginStatus=-1;RSSPSSOHandle *handle=rss_psso_begin(user?1:0,(uint32_t)method,generation,signingPoint.bytes,encryptionPoint.bytes,&beginStatus);
        // Serialize publication with ConfigurationRemoved before any Register can leave the process.
        __block BOOL submit=NO;
        dispatch_sync(dispatch_get_main_queue(), ^{
            if(handle && generation==self.generation && self.registrationCompletion) {
                NSString *metadata=field(handle,RSSMetadata);
                submit=RSSSetInflight(NSUserDefaults.standardUserDefaults,user?@"registration":@"deviceRegistration",metadata);
                if(submit)self.registrationMetadata=metadata;
            }
        });
        int result=submit?rss_psso_register(handle,signMessage,(void *)signing):-1;CFRelease(signing);
        dispatch_async(dispatch_get_main_queue(), ^{
            if (generation==self.generation && self.registrationCompletion) {
                NSError *error=nil;BOOL saved=NO;
                if (result==0 && RSSBeginSave(NSUserDefaults.standardUserDefaults,user?@"registration":@"deviceRegistration",field(handle,RSSMetadata))) {
                    if (user) {
                        ASAuthorizationProviderExtensionUserLoginConfiguration *config=[[ASAuthorizationProviderExtensionUserLoginConfiguration alloc]initWithLoginUserName:field(handle,RSSLoginName)];
                        saved=[manager saveUserLoginConfiguration:config error:&error];
                    } else {
                        ASAuthorizationProviderExtensionLoginConfiguration *config=[[ASAuthorizationProviderExtensionLoginConfiguration alloc]initWithClientID:field(handle,RSSClient) issuer:field(handle,RSSIssuer) tokenEndpointURL:[NSURL URLWithString:field(handle,RSSToken)] jwksEndpointURL:[NSURL URLWithString:field(handle,RSSJwks)] audience:field(handle,RSSAudience)];
                        config.nonceEndpointURL=[NSURL URLWithString:field(handle,RSSNonce)];config.nonceResponseKeypath=@"nonce";config.serverNonceClaimName=@"request_nonce";
                        config.additionalScopes=@"urn:apple:platformsso";
                        config.customLoginRequestValues=@[[NSURLQueryItem queryItemWithName:@"platform_sso_version" value:@"1.0"]];
                        [config setCustomLoginRequestHeaderClaims:@{@"typ":field(handle,RSSLoginType)} returningError:&error];
                        if(!error)[config setCustomAssertionRequestHeaderClaims:@{@"typ":field(handle,RSSAssertionType)} returningError:&error];
                        saved=!error && [manager saveLoginConfiguration:config error:&error];
                    }
                }
                if(saved)RSSSetInflight(NSUserDefaults.standardUserDefaults,user?@"registration":@"deviceRegistration",nil);
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
- (void)registrationDidCancel {
    NSUInteger generation=self.generation;[self finishRegistration:generation result:ASAuthorizationProviderExtensionRegistrationResultFailed];self.generation++;self.manager=nil;
    NSString *metadata=self.registrationMetadata;self.registrationMetadata=nil;
    if(RSSQueueCancelledRegistration(NSUserDefaults.standardUserDefaults,self.registrationScope,metadata))
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY,0), ^{rss_psso_unregister(metadata.UTF8String);});
}
- (void)observeMetadata:(NSString *)metadata scope:(NSString *)scope configuration:(NSString *)actual device:(BOOL)device user:(BOOL)user remaining:(NSUInteger)remaining {
    if(!remaining || !RSSObservationCurrent(NSUserDefaults.standardUserDefaults,scope,metadata))return;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY,0), ^{
        if(rss_psso_observe(metadata.UTF8String,actual.UTF8String,device,user)==0)return;
        // Retry this actual snapshot and source generation, never reconstruct expected facts.
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW,NSEC_PER_SEC),dispatch_get_main_queue(), ^{
            [self observeMetadata:metadata scope:scope configuration:actual device:device user:user remaining:remaining-1];
        });
    });
}
- (void)observeUsing:(ASAuthorizationProviderExtensionLoginManager *)manager {
    if(!manager)return;
    NSString *actual=nativeConfiguration(manager);
    BOOL device=manager.isDeviceRegistered,user=manager.isUserRegistered;
    for(NSString *scope in RSSRegistrationScopes()) {
        NSString *metadata=[NSUserDefaults.standardUserDefaults stringForKey:scope];
        if(metadata)[self observeMetadata:metadata scope:scope configuration:actual device:device user:user remaining:6];
    }
}
- (void)registrationDidComplete { [self observeUsing:self.manager];self.manager=nil;self.registrationMetadata=nil; }
- (void)retireUsing:(ASAuthorizationProviderExtensionLoginManager *)manager completion:(void (^)(BOOL))completion {
    NSDictionary<NSString *,NSString *> *pending=RSSPendingRetirements(NSUserDefaults.standardUserDefaults);
    if(!pending.count){completion(YES);return;}
    if(!manager){completion(NO);return;}
    BOOL device=manager.isDeviceRegistered,user=manager.isUserRegistered;
    NSString *actual=nativeConfiguration(manager);
    NSMutableSet<NSString *> *attempted=[NSMutableSet set];
    for(NSString *scope in RSSRegistrationScopes()) { NSString *metadata=[NSUserDefaults.standardUserDefaults stringForKey:scope];if(metadata)[attempted addObject:metadata]; }
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY,0), ^{
        NSMutableArray<NSString *> *completed=[NSMutableArray array];
        for(NSString *metadata in pending) {
            NSString *scope=pending[metadata];
            BOOL observed=rss_psso_observe(metadata.UTF8String,actual.UTF8String,device,user)==0;
            BOOL retired=rss_psso_unregister(metadata.UTF8String)==0;
            BOOL nativeCleared=![attempted containsObject:metadata] || (!user && (![scope isEqual:@"deviceRegistration"] || !device));
            if(retired && nativeCleared && (observed || ![attempted containsObject:metadata]))[completed addObject:metadata];
        }
        dispatch_async(dispatch_get_main_queue(), ^{
            for(NSString *metadata in completed)RSSCompleteRetirement(NSUserDefaults.standardUserDefaults,pending[metadata],metadata);
            completion(RSSPendingRetirements(NSUserDefaults.standardUserDefaults).count==0);
        });
    });
}
- (void)beginAuthorizationWithRequest:(ASAuthorizationProviderExtensionAuthorizationRequest *)request {
    [self observeUsing:request.loginManager];
    if ([request.requestedOperation isEqual:ASAuthorizationProviderAuthorizationOperationConfigurationRemoved] || RSSPendingRetirements(NSUserDefaults.standardUserDefaults).count) {
        if(self.registrationCompletion)[self registrationDidCancel];
        if(self.authorization)[self cancelAuthorizationWithRequest:self.authorization];
        RSSQueueRetirements(NSUserDefaults.standardUserDefaults);
        if(request.loginManager) {
            request.loginManager.ssoTokens=nil;
            if(request.loginManager.authenticationMethod==ASAuthorizationProviderExtensionAuthenticationMethodUserSecureEnclaveKey)[request.loginManager resetUserSecureEnclaveKey];
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
    ASAuthorizationProviderExtensionAuthenticationMethod method=manager.authenticationMethod;
    if(method!=ASAuthorizationProviderExtensionAuthenticationMethodPassword && method!=ASAuthorizationProviderExtensionAuthenticationMethodUserSecureEnclaveKey){[request completeWithError:failure()];return;}
    SecKeyRef signing=[manager copyKeyForKeyType:method==ASAuthorizationProviderExtensionAuthenticationMethodPassword ? ASAuthorizationProviderExtensionKeyTypeCurrentDeviceSigning : ASAuthorizationProviderExtensionKeyTypeUserSecureEnclaveKey];
    NSData *signingPoint=publicPoint(signing);
    if(!signingPoint){if(signing)CFRelease(signing);[request completeWithError:failure()];return;}
    self.authorization=request;NSUInteger generation=++self.generation;
    NSString *issuer=manager.loginConfiguration.issuer;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED,0), ^{
        char *bytes=rss_psso_browser((uint32_t)method,signingPoint.bytes,metadata.UTF8String,issuer.UTF8String,request.url.absoluteString.UTF8String,body.UTF8String,token.UTF8String,signMessage,(void *)signing);CFRelease(signing);
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
