// Native key operations shared with the ABI test; no private key export.
#import <Foundation/Foundation.h>
#import <Security/Security.h>
static NSData *publicPoint(SecKeyRef key) {
    if (!key) return nil;
    SecKeyRef publicKey=SecKeyCopyPublicKey(key);
    if (!publicKey) return nil;
    CFDataRef bytes=SecKeyCopyExternalRepresentation(publicKey,NULL); CFRelease(publicKey);
    NSData *result=CFBridgingRelease(bytes);
    return result.length==65 ? result : nil;
}
static int32_t signMessage(void *context,const uint8_t *bytes,size_t length,uint8_t *output) {
    if (!context || !bytes || !output || length>65536) return -1;
    SecKeyRef key=(SecKeyRef)context;
    if (!SecKeyIsAlgorithmSupported(key,kSecKeyOperationTypeSign,kSecKeyAlgorithmECDSASignatureMessageRFC4754SHA256)) return -1;
    NSData *input=[NSData dataWithBytes:bytes length:length];
    CFDataRef signedBytes=SecKeyCreateSignature(key,kSecKeyAlgorithmECDSASignatureMessageRFC4754SHA256,(__bridge CFDataRef)input,NULL);
    NSData *signature=CFBridgingRelease(signedBytes);
    if (signature.length!=64) return -1;
    memcpy(output,signature.bytes,64); return 0;
}
