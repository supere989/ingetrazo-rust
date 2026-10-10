# Explicit native layer sRGB source contract candidate

Actual paired Intel evidence before this change: QPainter-owned window fourRGB20,51,89 versus nativeMetal-owned window fourRGB21,51,89 on the same runner. ARM paired captures match20,51,89. Both rawproducer controls are20,51,89. Prior actual failures remain retained unchanged.

The nativeUI/CAD shaders produce encoded sRGB bytes. The previous CAMetalLayer leaves colorspace nil; Apple documents that nil output is not color matched. This candidate identifies encoded contents with CGColorSpaceSRGB. It changes no clearRGBA, shader, authoredpalette, image tolerance, GPUownership, callbackcondition, deadline, or extra pass/copy. CAMetalLayer retains the color-space object. Failedcreation refuses attachment.

Official source: https://developer.apple.com/documentation/quartzcore/cametallayer/colorspace

BothmacOSarchitectures all ingetrazo-metal targets crosscheckPASS; actualMac same-runnerQt/nativecapture and existingapplicationpresentation/lifecycle gates still required. No cause/resolution claim at source commit time.
