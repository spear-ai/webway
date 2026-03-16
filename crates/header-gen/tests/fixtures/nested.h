/* nested.h — struct containing another struct */
#ifndef NESTED_H
#define NESTED_H

typedef struct Point {
    int x;
    int y;
} Point;

typedef struct Rect {
    Point top_left;
    Point bottom_right;
    int   border_width;
} Rect;

#endif /* NESTED_H */
