/* typedefs.h — typedef aliases should resolve transparently */
#ifndef TYPEDEFS_H
#define TYPEDEFS_H

typedef int   MyInt;
typedef float MyFloat;

typedef struct TypedefAlias {
    MyInt   value;
    MyFloat weight;
    int     count;
} TypedefAlias;

#endif /* TYPEDEFS_H */
