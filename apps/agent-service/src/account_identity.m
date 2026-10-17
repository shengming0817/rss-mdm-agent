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
