// ref: Apple configd/SystemConfiguration.fproj/SCNetworkProtocol.c@585b7f2fca293f4642d21d15c5daf187f63c4796.
// ref: Apple Quartz Display Services (CGDisplayConfiguration.h), CoreAudio/AudioHardware.h (SDK 26.4).
#import <Foundation/Foundation.h>
#import <AppKit/AppKit.h>
#import <SystemConfiguration/SystemConfiguration.h>
#import <CoreGraphics/CoreGraphics.h>
#import <CoreAudio/CoreAudio.h>
#import <IOKit/ps/IOPowerSources.h>
#import <IOKit/ps/IOPSKeys.h>
#import <AudioToolbox/AudioServices.h>
static NSDictionary *field(NSString *key, id value, NSString *reason) {
    return @{@"key":key,@"value":value?:NSNull.null,@"quality":value?@"complete":@"unknown",@"reason":reason?:NSNull.null};
}
static NSDictionary *target(NSString *identity, NSString *name, NSArray *fields, NSArray *options, BOOL writable, NSString *reason) {
    return @{@"id":identity,@"name":name,@"fields":fields,@"options":options,@"writable":@(writable),@"reason":reason?:NSNull.null};
}
static id value(NSDictionary *t, NSString *key) { for (NSDictionary *f in t[@"fields"]) if ([f[@"key"] isEqual:key]) return f[@"value"] == NSNull.null ? nil : f[@"value"]; return nil; }
static BOOL forced(NSString *domain, NSArray *keys) { for (NSString *key in keys) if (CFPreferencesAppValueIsForced((__bridge CFStringRef)key,(__bridge CFStringRef)domain)) return YES; return NO; }
static NSString *displayID(CGDirectDisplayID display) { CFUUIDRef uuid=CGDisplayCreateUUIDFromDisplayID(display); if (!uuid) return nil; NSString *s=CFBridgingRelease(CFUUIDCreateString(NULL,uuid)); CFRelease(uuid); return s; }
static NSString *modeID(CGDisplayModeRef mode) { return [NSString stringWithFormat:@"%u",CGDisplayModeGetIODisplayModeID(mode)]; }
static NSArray *displays(void) {
    CGDirectDisplayID ids[64]; uint32_t count=0; if (CGGetOnlineDisplayList(64,ids,&count)!=kCGErrorSuccess) return nil;
    NSMutableArray *result=[NSMutableArray array];
    for (uint32_t i=0;i<count;i++) {
        NSString *identity=displayID(ids[i]); CGDisplayModeRef active=CGDisplayCopyDisplayMode(ids[i]); if (!identity || !active) return nil;
        NSArray *modes=CFBridgingRelease(CGDisplayCopyAllDisplayModes(ids[i],(__bridge CFDictionaryRef)@{(__bridge NSString *)kCGDisplayShowDuplicateLowResolutionModes:@YES}));
        NSMutableArray *options=[NSMutableArray array]; NSMutableSet *seen=[NSMutableSet set];
        for (id item in modes) { CGDisplayModeRef m=(__bridge CGDisplayModeRef)item; NSString *mid=modeID(m); if ([seen containsObject:mid]) continue; [seen addObject:mid];
            if (options.count>=256) {CGDisplayModeRelease(active);return nil;}
            NSString *label=[NSString stringWithFormat:@"%zu × %zu (%zu × %zu pixels, %.0f Hz)%@",CGDisplayModeGetWidth(m),CGDisplayModeGetHeight(m),CGDisplayModeGetPixelWidth(m),CGDisplayModeGetPixelHeight(m),CGDisplayModeGetRefreshRate(m),CGDisplayModeGetPixelWidth(m)>CGDisplayModeGetWidth(m)?@" HiDPI":@""];
            [options addObject:@{@"id":mid,@"label":label}]; }
        CGRect bounds=CGDisplayBounds(ids[i]); BOOL managed=forced(@"com.apple.windowserver",@[@"DisplaySets",@"DisplayAnyUserSets"]);
        NSArray *fields=@[field(@"mode",modeID(active),nil),field(@"width",@(CGDisplayModeGetWidth(active)),nil),field(@"height",@(CGDisplayModeGetHeight(active)),nil),field(@"pixelWidth",@(CGDisplayModeGetPixelWidth(active)),nil),field(@"pixelHeight",@(CGDisplayModeGetPixelHeight(active)),nil),field(@"origin",@{@"x":@((int)bounds.origin.x),@"y":@((int)bounds.origin.y)},nil),field(@"main",@(ids[i]==CGMainDisplayID()),nil)];
        [result addObject:target(identity,[NSString stringWithFormat:@"显示器 %u%@",ids[i],CGDisplayIsBuiltin(ids[i])?@"（内置）":@""],fields,options,!managed,managed?@"由组织策略管理":nil)]; CGDisplayModeRelease(active);
    }
    [result sortUsingComparator:^NSComparisonResult(NSDictionary *a,NSDictionary *b){return [a[@"id"] compare:b[@"id"]];}];return result;
}
static NSDictionary *endpoint(NSDictionary *config, NSString *prefix) {
    return @{@"enabled":@([config[[prefix stringByAppendingString:@"Enable"]] boolValue]),@"host":config[[prefix stringByAppendingString:@"Proxy"]]?:@"",@"port":config[[prefix stringByAppendingString:@"Port"]]?:@0};
}
static NSArray *proxies(SCPreferencesRef preferences) {
    NSArray *services=CFBridgingRelease(SCNetworkServiceCopyAll(preferences)); if (!services || services.count>64) return nil;
    NSMutableArray *result=[NSMutableArray array];
    for (id item in services) { SCNetworkServiceRef service=(__bridge SCNetworkServiceRef)item; SCNetworkProtocolRef protocol=SCNetworkServiceCopyProtocol(service,kSCNetworkProtocolTypeProxies); if (!protocol) continue;
        NSDictionary *config=(__bridge NSDictionary *)SCNetworkProtocolGetConfiguration(protocol); config=config?:@{};
        BOOL managed=forced(@"com.apple.SystemConfiguration",@[@"Proxies",@"HTTPEnable",@"HTTPSEnable",@"SOCKSEnable"]);
        NSArray *fields=@[field(@"http",endpoint(config,@"HTTP"),nil),field(@"https",endpoint(config,@"HTTPS"),nil),field(@"socks",endpoint(config,@"SOCKS"),nil),field(@"bypass",config[(__bridge NSString *)kSCPropNetProxiesExceptionsList]?:@[],nil),field(@"pacEnabled",@([config[(__bridge NSString *)kSCPropNetProxiesProxyAutoConfigEnable] boolValue]),nil),field(@"wpadEnabled",@([config[(__bridge NSString *)kSCPropNetProxiesProxyAutoDiscoveryEnable] boolValue]),nil),field(@"enabled",@(SCNetworkServiceGetEnabled(service)),nil)];
        [result addObject:target((__bridge NSString *)SCNetworkServiceGetServiceID(service),(__bridge NSString *)SCNetworkServiceGetName(service)?:@"网络服务",fields,@[],!managed,managed?@"由组织策略管理":nil)]; CFRelease(protocol);
    }
    SCDynamicStoreRef store=SCDynamicStoreCreate(NULL,CFSTR("RSS active network read"),NULL,NULL);
    NSDictionary *active=store?CFBridgingRelease(SCDynamicStoreCopyValue(store,CFSTR("State:/Network/Global/IPv4"))):nil;
    id interface=[active[@"PrimaryInterface"] isKindOfClass:NSString.class]?active[@"PrimaryInterface"]:nil;
    id service=[active[@"PrimaryService"] isKindOfClass:NSString.class]?active[@"PrimaryService"]:nil;
    NSDictionary *ipv4=store&&service?CFBridgingRelease(SCDynamicStoreCopyValue(store,(__bridge CFStringRef)[NSString stringWithFormat:@"State:/Network/Service/%@/IPv4",service])):nil;
    [result addObject:target(@"active-network",@"当前 IPv4 默认路由",@[field(@"interface",interface,interface?nil:@"未取得当前 IPv4 默认接口"),field(@"addresses",ipv4[@"Addresses"],ipv4?nil:@"未取得默认接口地址")],@[],NO,nil)];
    if(store)CFRelease(store);
    [result sortUsingComparator:^NSComparisonResult(NSDictionary *a,NSDictionary *b){return [a[@"id"] compare:b[@"id"]];}];return result;
}
static int proxyApply(NSDictionary *request) {
    SCPreferencesRef preferences=SCPreferencesCreate(NULL,CFSTR("RSS local settings"),NULL); if(!preferences)return 1;
    if(!SCPreferencesLock(preferences,FALSE)){CFRelease(preferences);return 2;}
    int status=3; NSDictionary *patch=request[@"patch"];
    NSArray *current=proxies(preferences);
    if ([current isEqual:request[@"before"]]) {
        for(NSDictionary *t in current) if ([t[@"id"] isEqual:patch[@"target"]] && [t[@"writable"] boolValue]) {
            SCNetworkServiceRef service=SCNetworkServiceCopy(preferences,(__bridge CFStringRef)t[@"id"]); SCNetworkProtocolRef protocol=service?SCNetworkServiceCopyProtocol(service,kSCNetworkProtocolTypeProxies):NULL;
            if(protocol) {
                NSDictionary *old=(__bridge NSDictionary *)SCNetworkProtocolGetConfiguration(protocol); NSMutableDictionary *next=old?[old mutableCopy]:[NSMutableDictionary dictionary];
                for(NSArray *pair in @[@[@"http",@"HTTP"],@[@"https",@"HTTPS"],@[@"socks",@"SOCKS"]]) { NSDictionary *p=patch[pair[0]]; if(![p isKindOfClass:NSDictionary.class])continue; NSString *prefix=pair[1]; next[[prefix stringByAppendingString:@"Enable"]]=p[@"enabled"]; next[[prefix stringByAppendingString:@"Proxy"]]=p[@"host"]; next[[prefix stringByAppendingString:@"Port"]]=p[@"port"]; }
                if([patch[@"bypass"] isKindOfClass:NSArray.class])next[(__bridge NSString *)kSCPropNetProxiesExceptionsList]=patch[@"bypass"];
                status=SCNetworkProtocolSetConfiguration(protocol,(__bridge CFDictionaryRef)next)&&SCPreferencesCommitChanges(preferences)&&SCPreferencesApplyChanges(preferences)?0:2;
                CFRelease(protocol);
            } if(service)CFRelease(service);
        }
    }
    SCPreferencesUnlock(preferences);CFRelease(preferences);return status;
}
static int displayConfigure(NSDictionary *request, BOOL restore, BOOL permanent) {
    NSDictionary *patch=request[@"patch"]; NSArray *before=request[@"before"];
    if(!restore && !permanent && ![displays() isEqual:before])return 3;
    CGDisplayConfigRef config=NULL; if(CGBeginDisplayConfiguration(&config)!=kCGErrorSuccess)return 1;
    int status=3; CGDirectDisplayID ids[64];uint32_t count=0;
    if(CGGetOnlineDisplayList(64,ids,&count)==kCGErrorSuccess) for(uint32_t i=0;i<count;i++) if([displayID(ids[i]) isEqual:patch[@"target"]]) {
        NSDictionary *original=nil;for(NSDictionary *t in before)if([t[@"id"] isEqual:patch[@"target"]])original=t;
        if(!original || ![original[@"writable"] boolValue])break;
        NSString *mid=restore?value(original,@"mode"):([patch[@"mode"] isKindOfClass:NSString.class]?patch[@"mode"]:nil);
        NSDictionary *origin=restore?value(original,@"origin"):([patch[@"origin"] isKindOfClass:NSDictionary.class]?patch[@"origin"]:nil);
        status=0;
        if(mid){BOOL found=NO;NSArray *modes=CFBridgingRelease(CGDisplayCopyAllDisplayModes(ids[i],(__bridge CFDictionaryRef)@{(__bridge NSString *)kCGDisplayShowDuplicateLowResolutionModes:@YES})); for(id item in modes){CGDisplayModeRef m=(__bridge CGDisplayModeRef)item;if([modeID(m) isEqual:mid]){found=YES;if(CGConfigureDisplayWithDisplayMode(config,ids[i],m,NULL)!=kCGErrorSuccess)status=1;break;}}if(!found)status=4;}
        if(origin && CGConfigureDisplayOrigin(config,ids[i],[origin[@"x"] intValue],[origin[@"y"] intValue])!=kCGErrorSuccess)status=1;
        break;
    }
    if(status==0){ if(CGCompleteDisplayConfiguration(config,permanent?kCGConfigurePermanently:(restore?kCGConfigureForSession:kCGConfigureForAppOnly))!=kCGErrorSuccess)status=1; }else CGCancelDisplayConfiguration(config);
    return status;
}
static int preserveDisplay(NSArray *expected) {
    CGDisplayConfigRef config=NULL;if(CGBeginDisplayConfiguration(&config)!=kCGErrorSuccess)return 1;
    NSArray *actual=displays();BOOL match=actual && actual.count==expected.count;
    for(NSDictionary *old in expected){NSDictionary *found=nil;for(NSDictionary *current in actual)if([current[@"id"] isEqual:old[@"id"]])found=current;if(!found || ![found[@"fields"] isEqual:old[@"fields"]])match=NO;}
    if(!match){CGCancelDisplayConfiguration(config);return 3;}
    CGDirectDisplayID ids[64];uint32_t count=0;int status=0;
    if(CGGetOnlineDisplayList(64,ids,&count)!=kCGErrorSuccess)status=1;
    for(uint32_t i=0;status==0&&i<count;i++){CGDisplayModeRef mode=CGDisplayCopyDisplayMode(ids[i]);CGRect bounds=CGDisplayBounds(ids[i]);if(!mode){status=1;break;}if(CGConfigureDisplayWithDisplayMode(config,ids[i],mode,NULL)!=kCGErrorSuccess || CGConfigureDisplayOrigin(config,ids[i],bounds.origin.x,bounds.origin.y)!=kCGErrorSuccess)status=1;CGDisplayModeRelease(mode);}
    if(status){CGCancelDisplayConfiguration(config);return status;}
    return CGCompleteDisplayConfiguration(config,kCGConfigureForSession)==kCGErrorSuccess?0:1;
}
static NSArray *audio(void) {
    NSMutableArray *fields=[NSMutableArray array];
    for(NSArray *pair in @[@[@"output",@(kAudioHardwarePropertyDefaultOutputDevice)],@[@"input",@(kAudioHardwarePropertyDefaultInputDevice)]]) {
        AudioObjectPropertyAddress p={[pair[1] unsignedIntValue],kAudioObjectPropertyScopeGlobal,kAudioObjectPropertyElementMain};AudioDeviceID device=0;UInt32 size=sizeof(device);
        if(AudioObjectGetPropertyData(kAudioObjectSystemObject,&p,0,NULL,&size,&device)==noErr && device!=kAudioObjectUnknown) {
            p.mSelector=kAudioObjectPropertyName; CFStringRef name=NULL;size=sizeof(name); if(AudioObjectGetPropertyData(device,&p,0,NULL,&size,&name)==noErr && name) [fields addObject:field(pair[0],CFBridgingRelease(name),nil)]; else [fields addObject:field(pair[0],nil,@"无法读取音频设备名称")];
            if([pair[0] isEqual:@"output"]){p.mSelector=kAudioHardwareServiceDeviceProperty_VirtualMainVolume;p.mScope=kAudioDevicePropertyScopeOutput;Float32 volume=0;size=sizeof(volume);[fields addObject:AudioObjectGetPropertyData(device,&p,0,NULL,&size,&volume)==noErr?field(@"volume",@(volume),nil):field(@"volume",nil,@"当前设备不提供主音量")];}
        }else [fields addObject:field(pair[0],nil,@"未取得默认音频设备")];
    }
    return @[target(@"audio",@"当前音频",fields,@[],NO,@"在系统设置中选择音频设备")];
}
static NSArray *readSettings(NSString *section) {
    if([section isEqual:@"display"])return displays();
    if([section isEqual:@"network"]){SCPreferencesRef p=SCPreferencesCreate(NULL,CFSTR("RSS settings read"),NULL);if(!p)return nil;NSArray *r=proxies(p);CFRelease(p);return r;}
    if([section isEqual:@"sound"])return audio();
    if([section isEqual:@"power"]){CFTypeRef info=IOPSCopyPowerSourcesInfo();if(!info)return @[target(@"power-source",@"供电状态",@[field(@"source",nil,@"无法读取供电来源")],@[],NO,nil)];NSString *source=(__bridge NSString *)IOPSGetProvidingPowerSourceType(info);NSArray *sources=CFBridgingRelease(IOPSCopyPowerSourcesList(info));BOOL battery=NO;for(id item in sources){NSDictionary *d=(__bridge NSDictionary *)IOPSGetPowerSourceDescription(info,(__bridge CFTypeRef)item);if([d[@kIOPSTypeKey] isEqual:@kIOPSInternalBatteryType])battery=YES;}NSArray *result=@[target(@"power-source",@"供电状态",@[field(@"source",source,nil),field(@"batteryPresent",@(battery),nil)],@[],NO,nil)];CFRelease(info);return result;}
    if([section isEqual:@"lock"]){CFPropertyListRef raw=CFPreferencesCopyAppValue(CFSTR("askForPassword"),CFSTR("com.apple.screensaver"));id v=CFBridgingRelease(raw);id login=[v isKindOfClass:NSNumber.class]?@([v boolValue]):nil;return @[target(@"login",@"返回电脑时",@[field(@"requireLogin",login,login?nil:@"系统未提供可确认的登录要求")],@[],NO,@"登录要求由系统与组织策略管理")];}
    return @[target(@"device",@"本机采集",@[field(@"facts",nil,@"通过本机采集目录读取此页状态")],@[],NO,@"请使用本机采集状态或打开系统设置")];
}
int rss_settings_call(const uint8_t *bytes,size_t length,uint8_t **output,size_t *size) {
    @autoreleasepool {
        if(length>65536 || !output || !size)return 1;*output=NULL;*size=0;
        NSDictionary *request=[NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:bytes length:length] options:0 error:NULL];if(![request isKindOfClass:NSDictionary.class])return 1;
        NSString *kind=request[@"kind"];id result=nil;int status=0;
        if([kind isEqual:@"read"])result=readSettings(request[@"section"]);
        else if([kind isEqual:@"powerManaged"])result=@(forced(@"com.apple.PowerManagement",@[@"AC Power",@"Battery Power",@"SystemPowerSettings"]));
        else if([kind isEqual:@"apply"]){NSString *p=request[@"patch"][@"kind"];status=[p isEqual:@"proxy"]?proxyApply(request):[p isEqual:@"display"]?displayConfigure(request,NO,NO):4;result=@YES;}
        else if([kind isEqual:@"preserveDisplay"]){status=preserveDisplay(request[@"before"]);result=@YES;}
        else if([kind isEqual:@"keepDisplay"] || [kind isEqual:@"restoreDisplay"]){status=displayConfigure(request,[kind isEqual:@"restoreDisplay"],[kind isEqual:@"keepDisplay"]);result=@YES;}
        else if([kind isEqual:@"open"]){NSDictionary *dest=@{@"network":@"com.apple.Network-Settings.extension",@"display":@"com.apple.Displays-Settings.extension",@"sound":@"com.apple.Sound-Settings.extension",@"power":@"com.apple.Battery-Settings.extension",@"lock":@"com.apple.Lock-Screen-Settings.extension",@"storage":@"com.apple.settings.Storage",@"security":@"com.apple.settings.PrivacySecurity.extension",@"about":@"com.apple.SystemProfiler.AboutExtension"};NSString *page=dest[request[@"section"]];if(!page)return 4;BOOL opened=[NSWorkspace.sharedWorkspace openURL:[NSURL URLWithString:[@"x-apple.systempreferences:" stringByAppendingString:page]]];result=@(opened);if(!opened)status=1;}
        else return 4;
        if(!result)return 1;NSData *data=[NSJSONSerialization dataWithJSONObject:result options:NSJSONWritingFragmentsAllowed error:NULL];if(!data || data.length>256*1024)return 1;*output=malloc(data.length);if(!*output)return 1;memcpy(*output,data.bytes,data.length);*size=data.length;return status;
    }
}
