/* packed.h — __attribute__((packed)) changes ABI byte size */
#ifndef PACKED_H
#define PACKED_H

typedef struct __attribute__((packed)) PackedRecord {
    char  tag;
    int   value;
    short flags;
} PackedRecord;

typedef struct AlignedRecord {
    char  tag;
    int   value;
    short flags;
} AlignedRecord;

#endif /* PACKED_H */
