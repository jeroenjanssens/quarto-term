#!/bin/sh
if [ -f recording-demo.termshow ]; then
  uvx great-docs termshow render recording-demo.termshow -o termshow/recording-demo
fi
