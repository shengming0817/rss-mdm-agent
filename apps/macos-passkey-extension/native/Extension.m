// ref: Apple AuthenticationServices/ASCredentialProviderViewController.h; LocalAuthentication/LAContext.h (SDK 26.4).
#import <AppKit/AppKit.h>
#import <AuthenticationServices/AuthenticationServices.h>
#import <LocalAuthentication/LocalAuthentication.h>
#import "Bridge.h"
static NSArray *numbers(NSData *data) {
    NSMutableArray *result=[NSMutableArray new];
    const uint8_t *bytes=data.bytes;
    for(NSUInteger i=0;i<data.length;i++) [result addObject:@(bytes[i])];
    return result;
}
static NSData *binary(NSArray *values) {
    NSMutableData *data=[NSMutableData dataWithLength:values.count];
    uint8_t *out=data.mutableBytes;
    for(NSUInteger i=0;i<values.count;i++) out[i]=[values[i] unsignedCharValue];
    return data;
}
static id consume(char *text) {
    if(!text) return nil;
    NSData *data=[[NSString stringWithUTF8String:text] dataUsingEncoding:NSUTF8StringEncoding];
    rss_passkey_free_text(text);
    return [NSJSONSerialization JSONObjectWithData:data options:0 error:NULL];
}
@interface RSSPasskeyProvider : ASCredentialProviderViewController
@property RSSPasskeyHandle *handle;
@property NSDictionary *request;
@property NSArray *accounts;
@property NSStackView *stack;
@property NSPopUpButton *chooser;
@property NSSecureTextField *password;
@property LAContext *authentication;
@property NSTimer *deadline;
@property NSMutableArray *observers;
@property BOOL finished;
@property BOOL working;
@end
@implementation RSSPasskeyProvider
- (void)loadView {
    self.view=[[NSView alloc]initWithFrame:NSMakeRect(0,0,480,340)];
    self.stack=[NSStackView new]; self.stack.orientation=NSUserInterfaceLayoutOrientationVertical;
    self.stack.spacing=14; self.stack.translatesAutoresizingMaskIntoConstraints=NO;
    [self.view addSubview:self.stack];
    [NSLayoutConstraint activateConstraints:@[[self.stack.leadingAnchor constraintEqualToAnchor:self.view.leadingAnchor constant:24],
        [self.stack.trailingAnchor constraintEqualToAnchor:self.view.trailingAnchor constant:-24],
        [self.stack.topAnchor constraintEqualToAnchor:self.view.topAnchor constant:24]]];
}
- (void)clear {
    (void)self.view;
    for(NSView *view in self.stack.arrangedSubviews.copy){[self.stack removeArrangedSubview:view];[view removeFromSuperview];}
}
- (void)label:(NSString *)text { [self.stack addArrangedSubview:[NSTextField wrappingLabelWithString:text]]; }
- (void)button:(NSString *)text action:(SEL)action { [self.stack addArrangedSubview:[NSButton buttonWithTitle:text target:self action:action]]; }
- (void)fail:(NSInteger)code {
    if(self.finished)return;
    self.finished=YES; [self.deadline invalidate]; [self.authentication invalidate];
    if(self.handle)rss_passkey_cancel(self.handle);
    [self.extensionContext cancelRequestWithError:[NSError errorWithDomain:ASExtensionErrorDomain code:code userInfo:nil]];
}
- (void)cancel:(id)sender { (void)sender; [self fail:ASExtensionErrorCodeUserCanceled]; }
- (void)failure:(int)code {
    [self fail:code==1?ASExtensionErrorCodeUserCanceled:code==2?ASExtensionErrorCodeCredentialIdentityNotFound:ASExtensionErrorCodeFailed];
}
- (void)start:(NSDictionary *)request {
    if(self.request || self.finished){[self fail:ASExtensionErrorCodeFailed];return;}
    self.request=request;
    [self clear]; [self label:[NSString stringWithFormat:@"%@ · %@",[request[@"kind"] isEqual:@"register"]?@"创建 Passkey":@"使用 Passkey",request[@"rp"]]];
    [self label:@"此请求只使用当前用户的个人 Vault。请确认网站，解锁后选择账号并继续。"];
    __weak RSSPasskeyProvider *weak=self;
    self.deadline=[NSTimer scheduledTimerWithTimeInterval:120 repeats:NO block:^(NSTimer *timer){(void)timer;[weak cancel:nil];}];
    self.observers=[NSMutableArray new];
    for(NSString *name in @[NSWorkspaceSessionDidResignActiveNotification,NSWorkspaceWillSleepNotification,NSWorkspaceWillPowerOffNotification]) {
        [self.observers addObject:[NSWorkspace.sharedWorkspace.notificationCenter addObserverForName:name object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note){(void)note;[weak cancel:nil];}]];
    }
    for (NSString *name in @[@"com.apple.screenIsLocked", @"com.rss.mdmagent.personal-passkeys.lock"])
    [self.observers addObject:[NSDistributedNotificationCenter.defaultCenter addObserverForName:name object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note){(void)note;[weak cancel:nil];}]];
    NSData *input=[NSJSONSerialization dataWithJSONObject:request options:0 error:NULL];
    self.working=YES;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED,0),^{
        RSSPasskeyHandle *handle=NULL; int result=rss_passkey_begin(input.bytes,input.length,&handle);
        dispatch_async(dispatch_get_main_queue(),^{
            self.working=NO;
            if(self.finished){rss_passkey_free(handle);return;}
            if(result){[self failure:result];return;}
            self.handle=handle;
            [self button:@"系统确认并解锁" action:@selector(unlockSystem:)];
            self.password=[NSSecureTextField new]; self.password.placeholderString=@"或输入个人 Vault 口令";
            [self.stack addArrangedSubview:self.password];
            [self button:@"用个人口令解锁" action:@selector(unlockPassword:)];
        });
    });
    [self button:@"取消" action:@selector(cancel:)];
}
- (void)unlockSystem:(id)sender {
    (void)sender; if(self.working || self.finished)return; self.working=YES;
    self.authentication=[LAContext new];
    NSString *reason=[NSString stringWithFormat:@"为 %@ 授权此次个人 Passkey 请求",self.request[@"rp"]];
    [self.authentication evaluatePolicy:LAPolicyDeviceOwnerAuthentication localizedReason:reason reply:^(BOOL allowed,NSError *error){
        (void)error;
        dispatch_async(dispatch_get_main_queue(),^{
            if(self.finished)return;
            if(!allowed){[self cancel:nil];return;}
            [self unlockVerified:YES password:nil];
        });
    }];
}
- (void)unlockPassword:(id)sender {
    (void)sender; if(self.working || self.finished)return;
    NSMutableData *password=[[self.password.stringValue dataUsingEncoding:NSUTF8StringEncoding] mutableCopy];
    self.password.stringValue=@"";
    if(!password.length || password.length>1024){[self fail:ASExtensionErrorCodeFailed];return;}
    self.working=YES; [self unlockVerified:NO password:password];
}
- (void)unlockVerified:(BOOL)verified password:(NSMutableData *)password {
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED,0),^{
        int result=rss_passkey_unlock(self.handle,verified?1:0,password.bytes,password.length);
        if(password.length)memset_s(password.mutableBytes,password.length,0,password.length);
        char *text=NULL;
        if(!result)result=rss_passkey_accounts(self.handle,&text);
        NSArray *accounts=consume(text);
        dispatch_async(dispatch_get_main_queue(),^{
            self.working=NO; if(self.finished)return;
            if(result){[self failure:result];return;}
            self.accounts=accounts;
            [self clear]; [self label:self.request[@"rp"]];
            if([self.request[@"kind"] isEqual:@"register"]){
                [self label:[NSString stringWithFormat:@"创建账号：%@",self.request[@"username"]]];
            }else{
                if(!accounts.count){[self fail:ASExtensionErrorCodeCredentialIdentityNotFound];return;}
                self.chooser=[NSPopUpButton new];
                for(NSDictionary *account in accounts)[self.chooser addItemWithTitle:[NSString stringWithFormat:@"%@ · %@",account[@"displayName"],account[@"username"]]];
                [self.stack addArrangedSubview:self.chooser];
            }
            [self button:@"确认此请求" action:@selector(confirm:)]; [self button:@"取消" action:@selector(cancel:)];
        });
    });
}
- (void)confirm:(id)sender {
    (void)sender; if(self.finished || self.working)return; self.working=YES;
    NSData *selected=nil;
    if([self.request[@"kind"] isEqual:@"assert"]){
        NSInteger index=self.chooser.indexOfSelectedItem;
        if(index<0 || (NSUInteger)index>=self.accounts.count){[self fail:ASExtensionErrorCodeCredentialIdentityNotFound];return;}
        selected=binary(self.accounts[index][@"credentialId"]);
    }
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED,0),^{
        char *text=NULL; int result=rss_passkey_execute(self.handle,selected.bytes,selected.length,&text);
        NSDictionary *response=consume(text);
        dispatch_async(dispatch_get_main_queue(),^{
            if(self.finished)return;
            if(result){[self failure:result];return;}
            // This is the last synchronous cancellation check before handoff to the OS.
            int check=rss_passkey_check(self.handle);
            if(check){[self failure:check];return;}
            self.finished=YES; [self.deadline invalidate]; [self.authentication invalidate];
            NSData *hash=binary(self.request[@"hash"]); NSString *rp=self.request[@"rp"];
            if([self.request[@"kind"] isEqual:@"register"]){
                ASPasskeyRegistrationCredential *credential=[[ASPasskeyRegistrationCredential alloc]initWithRelyingParty:rp clientDataHash:hash credentialID:binary(response[@"credentialId"]) attestationObject:binary(response[@"attestationObject"])];
                [self.extensionContext completeRegistrationRequestWithSelectedPasskeyCredential:credential completionHandler:nil];
            }else{
                ASPasskeyAssertionCredential *credential=[[ASPasskeyAssertionCredential alloc]initWithUserHandle:binary(response[@"userHandle"]) relyingParty:rp signature:binary(response[@"signature"]) clientDataHash:hash authenticatorData:binary(response[@"authenticatorData"]) credentialID:binary(response[@"credentialId"])];
                [self.extensionContext completeAssertionRequestWithSelectedPasskeyCredential:credential completionHandler:nil];
            }
        });
    });
}
- (void)prepareInterfaceForPasskeyRegistration:(id<ASCredentialRequest>)request {
    if(![request isKindOfClass:ASPasskeyCredentialRequest.class]){[self fail:ASExtensionErrorCodeFailed];return;}
    ASPasskeyCredentialRequest *passkey=(ASPasskeyCredentialRequest *)request;
    if(passkey.type != ASCredentialRequestTypePasskeyRegistration || passkey.registrationExtensionInput.largeBlob){[self fail:ASExtensionErrorCodeFailed];return;}
    ASPasskeyCredentialIdentity *identity=(ASPasskeyCredentialIdentity *)passkey.credentialIdentity;
    if(![identity isKindOfClass:ASPasskeyCredentialIdentity.class]){[self fail:ASExtensionErrorCodeFailed];return;}
    NSMutableArray *excluded=[NSMutableArray new];
    for(ASAuthorizationPlatformPublicKeyCredentialDescriptor *entry in passkey.excludedCredentials)[excluded addObject:numbers(entry.credentialID)];
    [self start:@{@"kind":@"register",@"rp":identity.relyingPartyIdentifier,@"hash":numbers(passkey.clientDataHash),@"user_handle":numbers(identity.userHandle),@"username":identity.userName,@"algorithms":passkey.supportedAlgorithms,@"exclude":excluded}];
}
- (void)prepareInterfaceToProvideCredentialForRequest:(id<ASCredentialRequest>)request {
    if(![request isKindOfClass:ASPasskeyCredentialRequest.class]){[self fail:ASExtensionErrorCodeFailed];return;}
    ASPasskeyCredentialRequest *passkey=(ASPasskeyCredentialRequest *)request;
    if(passkey.type != ASCredentialRequestTypePasskeyAssertion || passkey.assertionExtensionInput.largeBlob){[self fail:ASExtensionErrorCodeFailed];return;}
    ASPasskeyCredentialIdentity *identity=(ASPasskeyCredentialIdentity *)passkey.credentialIdentity;
    if(![identity isKindOfClass:ASPasskeyCredentialIdentity.class]){[self fail:ASExtensionErrorCodeFailed];return;}
    [self start:@{@"kind":@"assert",@"rp":identity.relyingPartyIdentifier,@"hash":numbers(passkey.clientDataHash),@"allowed":@[numbers(identity.credentialID)],@"identity":@{@"credential_id":numbers(identity.credentialID),@"user_handle":numbers(identity.userHandle)}}];
}
- (void)prepareCredentialListForServiceIdentifiers:(NSArray<ASCredentialServiceIdentifier *> *)identifiers requestParameters:(ASPasskeyCredentialRequestParameters *)parameters {
    (void)identifiers;
    if(parameters.extensionInput.largeBlob){[self fail:ASExtensionErrorCodeFailed];return;}
    NSMutableArray *allowed=[NSMutableArray new]; for(NSData *idValue in parameters.allowedCredentials)[allowed addObject:numbers(idValue)];
    [self start:@{@"kind":@"assert",@"rp":parameters.relyingPartyIdentifier,@"hash":numbers(parameters.clientDataHash),@"allowed":allowed,@"identity":NSNull.null}];
}
- (void)provideCredentialWithoutUserInteractionForRequest:(id<ASCredentialRequest>)request { (void)request; [self fail:ASExtensionErrorCodeUserInteractionRequired]; }
- (void)performPasskeyRegistrationWithoutUserInteractionIfPossible:(ASPasskeyCredentialRequest *)request { (void)request; [self fail:ASExtensionErrorCodeFailed]; }
- (void)prepareCredentialListForServiceIdentifiers:(NSArray<ASCredentialServiceIdentifier *> *)identifiers { (void)identifiers; [self fail:ASExtensionErrorCodeFailed]; }
- (void)prepareInterfaceForExtensionConfiguration {
    [self clear]; [self label:@"在 RSS 桌面设置中创建个人 Vault，然后在系统设置中启用此 Provider。仅本机保存，丢失后无管理员恢复入口。"];
    [self button:@"完成" action:@selector(configured:)];
}
- (void)configured:(id)sender { (void)sender;self.finished=YES;[self.extensionContext completeExtensionConfigurationRequest]; }
- (void)viewDidDisappear { [super viewDidDisappear]; if(!self.finished)[self cancel:nil]; }
- (void)dealloc {
    [self.deadline invalidate]; [self.authentication invalidate];
    for(id observer in self.observers){[NSWorkspace.sharedWorkspace.notificationCenter removeObserver:observer];[NSDistributedNotificationCenter.defaultCenter removeObserver:observer];}
    rss_passkey_free(self.handle);
}
@end
