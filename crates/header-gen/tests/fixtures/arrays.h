/* arrays.h — fixed-size array fields */
#ifndef ARRAYS_H
#define ARRAYS_H

typedef struct Arrays {
    char    name[32];
    int     values[8];
    float   coords[3];
    unsigned char raw[16];
} Arrays;

#endif /* ARRAYS_H */
