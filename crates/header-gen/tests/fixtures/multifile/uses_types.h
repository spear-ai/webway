/* uses_types.h — references a type from types.h */
#ifndef USES_TYPES_H
#define USES_TYPES_H

#include "types.h"

typedef struct Event {
    int       id;
    Timestamp created_at;
    char      name[64];
} Event;

#endif /* USES_TYPES_H */
