#import <AppKit/AppKit.h>
#import <QuickLookThumbnailing/QuickLookThumbnailing.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

// ARC keeps this heap state alive through late callbacks, including after a timeout.
@interface NetjesThumbnailResult : NSObject
@property(nonatomic, strong) dispatch_semaphore_t ready;
@property(nonatomic, strong) NSData *png;
@property(nonatomic) BOOL abandoned;
@end
@implementation NetjesThumbnailResult
@end

// Called on a Rust blocking worker, never the UI thread. Only the returned malloc buffer
// crosses the FFI boundary; the asynchronous block never captures Rust memory or outLength.
uint8_t *netjes_macos_thumbnail(const char *path, uint32_t px, uint32_t timeoutMs,
                              size_t *outLength) {
    if (!outLength) return NULL;
    *outLength = 0;
    if (!path || px == 0 || px > 1024 || timeoutMs > 5000) return NULL;
    @autoreleasepool {
        @try {
            NSString *name = [[NSFileManager defaultManager]
                stringWithFileSystemRepresentation:path length:strlen(path)];
            if (!name) return NULL;
            NSURL *url = [NSURL fileURLWithPath:name];
            QLThumbnailGenerationRequest *request = [[QLThumbnailGenerationRequest alloc]
                initWithFileAtURL:url size:CGSizeMake(px, px) scale:1.0
                representationTypes:QLThumbnailGenerationRequestRepresentationTypeThumbnail];
            QLThumbnailGenerator *generator = [QLThumbnailGenerator sharedGenerator];
            NetjesThumbnailResult *result = [NetjesThumbnailResult new];
            result.ready = dispatch_semaphore_create(0);
            [generator generateBestRepresentationForRequest:request
                completionHandler:^(QLThumbnailRepresentation *thumbnail, NSError *error) {
                    @autoreleasepool {
                        @synchronized(result) {
                            if (result.abandoned) return;
                        }
                        NSData *png = nil;
                        @try {
                            // Quick Look may return a generic icon even for a thumbnail request.
                            if (!error && thumbnail.type == QLThumbnailRepresentationTypeThumbnail) {
                                CGImageRef image = thumbnail.CGImage;
                                if (image && CGImageGetWidth(image) <= 1024 && CGImageGetHeight(image) <= 1024) {
                                    NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithCGImage:image];
                                    png = [bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
                                }
                            }
                        } @catch (NSException *exception) {
                            png = nil;
                        }
                        @synchronized(result) {
                            if (!result.abandoned) result.png = png;
                        }
                        dispatch_semaphore_signal(result.ready);
                    }
                }];
            long timedOut = dispatch_semaphore_wait(result.ready,
                dispatch_time(DISPATCH_TIME_NOW, (int64_t)timeoutMs * NSEC_PER_MSEC));
            NSData *png = nil;
            @synchronized(result) {
                if (!timedOut && timeoutMs != 0) png = result.png;
                result.abandoned = YES;
                result.png = nil;
            }
            if (timedOut || timeoutMs == 0) {
                [generator cancelRequest:request];
                return NULL;
            }
            if (!png.length || png.length > 16 * 1024 * 1024) return NULL;
            uint8_t *bytes = malloc(png.length);
            if (!bytes) return NULL;
            memcpy(bytes, png.bytes, png.length);
            *outLength = png.length;
            return bytes;
        } @catch (NSException *exception) {
            return NULL;
        }
    }
}

void netjes_macos_thumbnail_free(uint8_t *bytes) {
    free(bytes);
}
