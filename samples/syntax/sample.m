// Objective-C Syntax Highlighting Test
// A task manager with categories, blocks, protocols, and KVO.

#import <Foundation/Foundation.h>

// ============================================================
// Constants
// ============================================================

static NSString *const kAppVersion = @"1.0.0";
static NSInteger const kMaxTasks = 10000;
static NSTimeInterval const kAutoSaveInterval = 30.0;

// ============================================================
// Enums
// ============================================================

typedef NS_ENUM(NSUInteger, TEPriority) {
    TEPriorityLow = 0,
    TEPriorityMedium,
    TEPriorityHigh,
    TEPriorityCritical,
};

typedef NS_ENUM(NSUInteger, TEStatus) {
    TEStatusOpen = 0,
    TEStatusInProgress,
    TEStatusDone,
    TEStatusCancelled,
};

typedef NS_OPTIONS(NSUInteger, TETaskFilter) {
    TETaskFilterNone       = 0,
    TETaskFilterOpen       = 1 << 0,
    TETaskFilterInProgress = 1 << 1,
    TETaskFilterDone       = 1 << 2,
    TETaskFilterCancelled  = 1 << 3,
    TETaskFilterActive     = TETaskFilterOpen | TETaskFilterInProgress,
    TETaskFilterAll        = 0xFF,
};

// ============================================================
// Protocols
// ============================================================

@protocol TETaskDelegate <NSObject>
@required
- (void)taskDidChangeStatus:(id)task fromStatus:(TEStatus)oldStatus;
- (void)taskWasCreated:(id)task;
@optional
- (void)taskWasDeleted:(NSString *)taskId;
- (BOOL)shouldAllowTransitionFrom:(TEStatus)from to:(TEStatus)to;
@end

@protocol TESerializable <NSObject>
- (NSDictionary *)toDictionary;
+ (instancetype)fromDictionary:(NSDictionary *)dict;
@end

// ============================================================
// Task class
// ============================================================

@interface TETask : NSObject <TESerializable, NSCopying>

@property (nonatomic, copy, readonly) NSString *taskId;
@property (nonatomic, copy) NSString *title;
@property (nonatomic, copy) NSString *taskDescription;
@property (nonatomic, assign) TEStatus status;
@property (nonatomic, assign) TEPriority priority;
@property (nonatomic, strong) NSArray<NSString *> *tags;
@property (nonatomic, strong, readonly) NSDate *createdAt;
@property (nonatomic, strong) NSDate *updatedAt;

+ (instancetype)taskWithTitle:(NSString *)title priority:(TEPriority)priority;
+ (instancetype)taskWithTitle:(NSString *)title
                     priority:(TEPriority)priority
                         tags:(NSArray<NSString *> *)tags;

- (BOOL)transitionToStatus:(TEStatus)newStatus error:(NSError **)error;
- (NSComparisonResult)compareByPriority:(TETask *)other;
- (NSString *)formattedString;

@end

@implementation TETask

+ (instancetype)taskWithTitle:(NSString *)title priority:(TEPriority)priority {
    return [self taskWithTitle:title priority:priority tags:@[]];
}

+ (instancetype)taskWithTitle:(NSString *)title
                     priority:(TEPriority)priority
                         tags:(NSArray<NSString *> *)tags {
    TETask *task = [[self alloc] init];
    if (task) {
        task->_taskId = [[NSUUID UUID] UUIDString];
        task->_title = [title copy];
        task->_taskDescription = @"";
        task->_status = TEStatusOpen;
        task->_priority = priority;
        task->_tags = [tags copy];
        task->_createdAt = [NSDate date];
        task->_updatedAt = [NSDate date];
    }
    return task;
}

- (BOOL)transitionToStatus:(TEStatus)newStatus error:(NSError **)error {
    NSDictionary<NSNumber *, NSArray<NSNumber *> *> *validTransitions = @{
        @(TEStatusOpen):       @[@(TEStatusInProgress), @(TEStatusCancelled)],
        @(TEStatusInProgress): @[@(TEStatusOpen), @(TEStatusDone), @(TEStatusCancelled)],
        @(TEStatusDone):       @[@(TEStatusOpen)],
        @(TEStatusCancelled):  @[@(TEStatusOpen)],
    };

    NSArray *allowed = validTransitions[@(self.status)];
    if (![allowed containsObject:@(newStatus)]) {
        if (error) {
            *error = [NSError errorWithDomain:@"TETaskError"
                                         code:1001
                                     userInfo:@{
                NSLocalizedDescriptionKey:
                    [NSString stringWithFormat:@"Cannot transition from %@ to %@",
                     [self statusString:self.status],
                     [self statusString:newStatus]]
            }];
        }
        return NO;
    }

    TEStatus oldStatus = self.status;
    self.status = newStatus;
    self.updatedAt = [NSDate date];
    return YES;
}

- (NSComparisonResult)compareByPriority:(TETask *)other {
    if (self.priority > other.priority) return NSOrderedAscending;
    if (self.priority < other.priority) return NSOrderedDescending;
    return [self.createdAt compare:other.createdAt];
}

- (NSString *)formattedString {
    NSString *icon = @[@"[ ]", @"[~]", @"[x]", @"[-]"][self.status];
    NSString *prio = @[@" ", @"!", @"!!", @"!!!"][self.priority];
    NSString *tagStr = self.tags.count > 0
        ? [NSString stringWithFormat:@" [%@]", [self.tags componentsJoinedByString:@", "]]
        : @"";

    return [NSString stringWithFormat:@"#%@ %@ %@ %@%@",
            [self.taskId substringToIndex:4], icon, prio, self.title, tagStr];
}

- (NSString *)statusString:(TEStatus)status {
    switch (status) {
        case TEStatusOpen:       return @"open";
        case TEStatusInProgress: return @"in_progress";
        case TEStatusDone:       return @"done";
        case TEStatusCancelled:  return @"cancelled";
    }
}

#pragma mark - TESerializable

- (NSDictionary *)toDictionary {
    return @{
        @"id": self.taskId,
        @"title": self.title,
        @"description": self.taskDescription,
        @"status": @(self.status),
        @"priority": @(self.priority),
        @"tags": self.tags,
        @"createdAt": self.createdAt.description,
    };
}

+ (instancetype)fromDictionary:(NSDictionary *)dict {
    TETask *task = [self taskWithTitle:dict[@"title"]
                              priority:[dict[@"priority"] unsignedIntegerValue]
                                  tags:dict[@"tags"]];
    task.status = [dict[@"status"] unsignedIntegerValue];
    return task;
}

#pragma mark - NSCopying

- (id)copyWithZone:(NSZone *)zone {
    TETask *copy = [[TETask allocWithZone:zone] init];
    copy->_taskId = [self.taskId copy];
    copy->_title = [self.title copy];
    copy->_taskDescription = [self.taskDescription copy];
    copy->_status = self.status;
    copy->_priority = self.priority;
    copy->_tags = [self.tags copy];
    copy->_createdAt = [self.createdAt copy];
    copy->_updatedAt = [self.updatedAt copy];
    return copy;
}

#pragma mark - NSObject

- (NSString *)description {
    return [NSString stringWithFormat:@"<TETask: %@ \"%@\" [%@]>",
            [self.taskId substringToIndex:4], self.title,
            [self statusString:self.status]];
}

- (BOOL)isEqual:(id)object {
    if (![object isKindOfClass:[TETask class]]) return NO;
    return [self.taskId isEqualToString:((TETask *)object).taskId];
}

- (NSUInteger)hash {
    return self.taskId.hash;
}

@end

// ============================================================
// Category: Statistics
// ============================================================

@interface NSArray (TETaskStatistics)
- (NSDictionary *)taskStatistics;
@end

@implementation NSArray (TETaskStatistics)

- (NSDictionary *)taskStatistics {
    NSUInteger total = self.count;
    NSCountedSet *byStatus = [NSCountedSet set];
    NSCountedSet *byPriority = [NSCountedSet set];
    __block NSUInteger doneCount = 0;

    [self enumerateObjectsUsingBlock:^(TETask *task, NSUInteger idx, BOOL *stop) {
        [byStatus addObject:@(task.status)];
        [byPriority addObject:@(task.priority)];
        if (task.status == TEStatusDone) doneCount++;
    }];

    CGFloat completionRate = total > 0 ? (CGFloat)doneCount / total * 100.0 : 0.0;

    return @{
        @"total": @(total),
        @"byStatus": byStatus,
        @"byPriority": byPriority,
        @"completionRate": @(completionRate),
    };
}

@end

// ============================================================
// Task Store with KVO
// ============================================================

@interface TETaskStore : NSObject <TETaskDelegate>

@property (nonatomic, strong, readonly) NSArray<TETask *> *allTasks;
@property (nonatomic, assign, readonly) NSUInteger count;

- (TETask *)createTaskWithTitle:(NSString *)title
                       priority:(TEPriority)priority
                           tags:(NSArray<NSString *> *)tags;
- (NSArray<TETask *> *)tasksMatchingFilter:(TETaskFilter)filter;
- (NSArray<TETask *> *)tasksWithTag:(NSString *)tag;
- (void)printReport;

@end

@implementation TETaskStore {
    NSMutableArray<TETask *> *_tasks;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _tasks = [NSMutableArray array];
    }
    return self;
}

- (TETask *)createTaskWithTitle:(NSString *)title
                       priority:(TEPriority)priority
                           tags:(NSArray<NSString *> *)tags {
    TETask *task = [TETask taskWithTitle:title priority:priority tags:tags];
    [_tasks addObject:task];
    return task;
}

- (NSArray<TETask *> *)allTasks {
    return [_tasks sortedArrayUsingSelector:@selector(compareByPriority:)];
}

- (NSUInteger)count {
    return _tasks.count;
}

- (NSArray<TETask *> *)tasksMatchingFilter:(TETaskFilter)filter {
    NSPredicate *predicate = [NSPredicate predicateWithBlock:
        ^BOOL(TETask *task, NSDictionary *bindings) {
            return (filter & (1 << task.status)) != 0;
        }];
    return [_tasks filteredArrayUsingPredicate:predicate];
}

- (NSArray<TETask *> *)tasksWithTag:(NSString *)tag {
    return [_tasks filteredArrayUsingPredicate:
        [NSPredicate predicateWithFormat:@"ANY tags == %@", tag]];
}

- (void)printReport {
    NSDictionary *stats = [self.allTasks taskStatistics];
    NSLog(@"\n=== Statistics ===");
    NSLog(@"Total: %@", stats[@"total"]);
    NSLog(@"Completion: %.1f%%", [stats[@"completionRate"] doubleValue]);
}

#pragma mark - TETaskDelegate

- (void)taskDidChangeStatus:(TETask *)task fromStatus:(TEStatus)oldStatus {
    NSLog(@"Task %@ changed status", task.taskId);
}

- (void)taskWasCreated:(TETask *)task {
    NSLog(@"Created: %@", task.title);
}

@end

// ============================================================
// Main
// ============================================================

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        NSLog(@"Task Manager v%@", kAppVersion);

        TETaskStore *store = [[TETaskStore alloc] init];

        // Create tasks using modern literal syntax
        [store createTaskWithTitle:@"Implement syntax highlighting"
                          priority:TEPriorityHigh
                              tags:@[@"feature", @"syntax"]];
        [store createTaskWithTitle:@"Fix cursor blinking"
                          priority:TEPriorityLow
                              tags:@[@"bug"]];
        [store createTaskWithTitle:@"Add split view"
                          priority:TEPriorityMedium
                              tags:@[@"feature", @"ui"]];

        // Transition tasks
        NSError *error = nil;
        TETask *task1 = store.allTasks.firstObject;
        [task1 transitionToStatus:TEStatusInProgress error:&error];

        // Display using block enumeration
        NSLog(@"\nAll tasks:");
        [store.allTasks enumerateObjectsUsingBlock:
            ^(TETask *task, NSUInteger idx, BOOL *stop) {
                NSLog(@"  %@", [task formattedString]);
            }];

        // Filter with predicate
        NSArray *features = [store tasksWithTag:@"feature"];
        NSLog(@"\nFeature tasks: %lu", (unsigned long)features.count);

        [store printReport];
    }
    return 0;
}
