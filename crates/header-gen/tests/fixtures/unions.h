/* unions.h — struct containing a union member */
#ifndef UNIONS_H
#define UNIONS_H

typedef union Variant {
    int   as_int;
    float as_float;
    char  as_bytes[4];
} Variant;

typedef struct WithUnion {
    int     id;
    Variant data;
    int     tag;
} WithUnion;

#endif /* UNIONS_H */
