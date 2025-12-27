This project is a port of ~/code/hyprmagnifier/ (hyprmagnifier). It currently works very well with two main issues:

1) The frame for magnifiying doesn't render as soon as the app is started. As soon as the pointer moves the magnifiying frame appears and works correctly
2) Attempts to make it render use a calculation that places the initial frame in a different location from the pointer. In a two monitor layout this causes the frame to appear either on the center of the other monitor or close by to the center. 

I've tried many times fixing this issue, and the fixes goes to either show the frame but in the wrong position, or show it correctly in the right position but only when the pointer moves. 

Initially this project was supposed to also provide a "live" view while rendering via screen capturing while zooming but this proved poorly executed mainly due to hyperland not providing the hooks necessary to implement correctly.

While you are making updates and changes, ensure that TESTING is a top priority, low cyclomatic complexity is reported via `pmat`, and you are using the TOYOTA way.
