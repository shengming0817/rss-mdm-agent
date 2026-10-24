// ref: Apple OpenDirectory ODQuery / kODAttributeTypeGUID. Local directory only.
#import <Foundation/Foundation.h>
#import <OpenDirectory/OpenDirectory.h>
#include <stdint.h>
#include <string.h>
int rss_account_generated_uid(uint32_t uid, uint8_t *output) {
    @autoreleasepool {
        NSError *error = nil;
        ODNode *node = [ODNode nodeWithSession:[ODSession defaultSession]
                    type:kODNodeTypeLocalNodes error:&error];
        if (!node || error) return -1;
        ODQuery *query = [ODQuery queryWithNode:node forRecordTypes:kODRecordTypeUsers
              attribute:kODAttributeTypeUniqueID matchType:kODMatchEqualTo
              queryValues:[NSString stringWithFormat:@"%u", uid]
              returnAttributes:@[kODAttributeTypeGUID] maximumResults:2 error:&error];
        if (!query || error) return -1;
        NSArray *records = [query resultsAllowingPartial:NO error:&error];
        if (error || records.count != 1) return -1;
        NSArray *values = [records[0] valuesForAttribute:kODAttributeTypeGUID error:&error];
        if (error || values.count != 1 || ![values[0] isKindOfClass:[NSString class]]) return -1;
        NSData *data = [values[0] dataUsingEncoding:NSUTF8StringEncoding];
        if (data.length != 36) return -1;
        memcpy(output, data.bytes, 36);
        return 0;
    }
}

// Account mutations use exact GeneratedUID records; names are only initial creation coordinates.
static ODRecord *rss_access_record(ODNode *node, NSString *attribute, NSString *value, NSError **error) {
    ODQuery *query=[ODQuery queryWithNode:node forRecordTypes:kODRecordTypeUsers attribute:attribute matchType:kODMatchEqualTo queryValues:value returnAttributes:@[kODAttributeTypeRecordName,kODAttributeTypeGUID,kODAttributeTypeUniqueID,kODAttributeTypeAuthenticationAuthority] maximumResults:2 error:error];
    NSArray *records=[query resultsAllowingPartial:NO error:error];
    if(!*error && records.count>1)*error=[NSError errorWithDomain:@"com.rss.account" code:1 userInfo:nil];
    return !*error && records.count==1 ? records[0] : nil;
}
int rss_access_account(const char *value, int byName, uint8_t *output, size_t capacity) {
    @autoreleasepool {
        NSError *error=nil;ODNode *node=[ODNode nodeWithSession:ODSession.defaultSession name:@"/Local/Default" error:&error];
        if(!node || error || !value || !output)return -1;
        ODRecord *record=rss_access_record(node,byName?kODAttributeTypeRecordName:kODAttributeTypeGUID,[NSString stringWithUTF8String:value],&error);
        if(error)return -1;if(!record)return -2;
        NSArray *ids=[record valuesForAttribute:kODAttributeTypeGUID error:&error],*uids=[record valuesForAttribute:kODAttributeTypeUniqueID error:&error];
        if(error || ids.count!=1 || uids.count!=1)return -1;
        ODRecord *admins=[node recordWithRecordType:kODRecordTypeGroups name:@"admin" attributes:nil error:&error];
        BOOL administrator=[admins isMemberRecord:record error:&error];if(error)return -1;
        NSArray *authorities=[record valuesForAttribute:kODAttributeTypeAuthenticationAuthority error:&error];
        NSInteger token=error?-1:0;
        if(!error)for(NSString *authority in authorities)if([authority containsString:@";SecureToken;"])token=1;
        NSData *data=[NSJSONSerialization dataWithJSONObject:@{@"name":record.recordName,@"generatedUid":[ids[0] lowercaseString],@"uid":@([uids[0] intValue]),@"administrator":@(administrator),@"secureToken":@(token)} options:0 error:NULL];
        if(!data || data.length>capacity)return -1;memcpy(output,data.bytes,data.length);return (int)data.length;
    }
}
int rss_access_role(const char *uuid,int administrator) {
    @autoreleasepool {
        NSError *error=nil;ODNode *node=[ODNode nodeWithSession:ODSession.defaultSession name:@"/Local/Default" error:&error];
        ODRecord *record=rss_access_record(node,kODAttributeTypeGUID,[NSString stringWithUTF8String:uuid],&error);
        ODRecord *admins=[node recordWithRecordType:kODRecordTypeGroups name:@"admin" attributes:nil error:&error];
        if(error || !record || !admins)return -1;
        BOOL isAdmin=[admins isMemberRecord:record error:&error];if(error)return -1;
        if(!administrator && isAdmin) {
            NSArray *members=[admins valuesForAttribute:kODAttributeTypeGroupMembers error:&error];
            if(error || members.count>256)return -2;
            BOOL alternate=NO;
            for(NSString *member in members) {
                if([member caseInsensitiveCompare:[NSString stringWithUTF8String:uuid]]==NSOrderedSame)continue;
                NSError *candidateError=nil;ODRecord *candidate=rss_access_record(node,kODAttributeTypeGUID,member,&candidateError);
                NSArray *uids=[candidate valuesForAttribute:kODAttributeTypeUniqueID error:&candidateError];
                NSArray *authorities=[candidate valuesForAttribute:kODAttributeTypeAuthenticationAuthority error:&candidateError];
                BOOL disabled=NO;for(NSString *authority in authorities)if([authority containsString:@";DisabledUser;"])disabled=YES;
                if(candidate && !candidateError && uids.count==1 && [uids[0] intValue]>=501 && !disabled){alternate=YES;break;}
            }
            if(!alternate)return -2;
            if(![admins removeMemberRecord:record error:&error] || error)return -1;
        }
        if(administrator && !isAdmin && ![admins addMemberRecord:record error:&error])return -1;
        return error?-1:0;
    }
}
