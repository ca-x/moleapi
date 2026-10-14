#import <Foundation/Foundation.h>
#import <ProtoRPC/ProtoRPC.h>
#import <GRPCClient/GRPCCallOptions.h>
#import "Fixture.pbrpc.h"
#import "models/Types.pbobjc.h"

static void Ensure(BOOL value, NSString *message) {
  if (!value) @throw [NSException exceptionWithName:@"RuntimeFixture" reason:message userInfo:nil];
}
@interface Responses : NSObject <GRPCProtoResponseHandler>
@property(nonatomic, strong) dispatch_queue_t dispatchQueue;
@property(nonatomic, strong) dispatch_semaphore_t done;
@property(nonatomic, strong) NSMutableArray<GPBMessage *> *messages;
@property(nonatomic, strong) NSError *failure;
@end
@implementation Responses
- (instancetype)init {
  if ((self = [super init])) {
    _dispatchQueue = dispatch_queue_create("fixture.responses", DISPATCH_QUEUE_SERIAL);
    _done = dispatch_semaphore_create(0);
    _messages = [NSMutableArray array];
  }
  return self;
}
- (void)didReceiveProtoMessage:(GPBMessage *)message {
  if (message) [_messages addObject:message];
}
- (void)didCloseWithTrailingMetadata:(NSDictionary *)metadata error:(NSError *)error {
  _failure = error;
  dispatch_semaphore_signal(_done);
}
- (void)verify:(NSArray<GPBMessage *> *)expected {
  Ensure(dispatch_semaphore_wait(_done, dispatch_time(DISPATCH_TIME_NOW, 15 * NSEC_PER_SEC)) == 0,
         @"RPC completion timeout");
  Ensure(_failure == nil, _failure.description ?: @"RPC failed");
  Ensure([_messages isEqualToArray:expected], @"Decoded protobuf fields or response order lost");
}
@end

int main(int argc, const char *argv[]) {
  @autoreleasepool {
    Ensure(argc == 2, @"Explicit loopback address required");
    Echo *expected = [Echo message];
    expected.text = @"鼹鼠 \"quotes\" & +";
    const unsigned char bytes[] = {0, 255, 128, 65};
    expected.payload = [NSData dataWithBytes:bytes length:sizeof(bytes)];
    expected.shade = Shade_Ready;
    expected.labelsArray = [NSMutableArray arrayWithArray:@[@"red", @"蓝"]];
    expected.note = @"detail";
    expected.observed = [GPBTimestamp message];
    expected.observed.seconds = 1234567;
    expected.observed.nanos = 123;
    Echo *other = [expected copy];
    other.code = 0;
    Ensure(expected.detailOneOfCase == Echo_Detail_OneOfCase_Note &&
           other.detailOneOfCase == Echo_Detail_OneOfCase_Code, @"Oneof zero presence lost");
    GRPCMutableCallOptions *options = [[GRPCMutableCallOptions alloc] init];
    options.timeout = 10;
    options.transportType = GRPCTransportTypeInsecure;
    EchoService *service = [[EchoService alloc] initWithHost:[NSString stringWithUTF8String:argv[1]]
                                              callOptions:options];
    Responses *unary = [[Responses alloc] init];
    GRPCUnaryProtoCall *call = [service callWithMessage:expected responseHandler:unary callOptions:options];
    [call start];
    [unary verify:@[expected]];
    Responses *watch = [[Responses alloc] init];
    GRPCUnaryProtoCall *watchCall = [service watchWithMessage:expected responseHandler:watch callOptions:options];
    [watchCall start];
    [watch verify:@[expected, other]];
    Responses *collect = [[Responses alloc] init];
    GRPCStreamingProtoCall *collectCall = [service collectWithResponseHandler:collect callOptions:options];
    [collectCall start];
    [collectCall writeMessage:expected];
    [collectCall writeMessage:other];
    [collectCall finish];
    [collect verify:@[other]];
    Responses *chat = [[Responses alloc] init];
    GRPCStreamingProtoCall *chatCall = [service chatWithResponseHandler:chat callOptions:options];
    [chatCall start];
    [chatCall writeMessage:expected];
    [chatCall writeMessage:other];
    [chatCall finish];
    [chat verify:@[expected, other]];
    puts("PASS Objective-C v2 unary/server/client/bidi");
  }
  return 0;
}
