/* bitfields.h — struct with bitfield members */
#ifndef BITFIELDS_H
#define BITFIELDS_H

typedef struct Flags {
    unsigned int active    : 1;
    unsigned int priority  : 3;
    unsigned int mode      : 4;
    unsigned int reserved  : 24;
} Flags;

#endif /* BITFIELDS_H */
