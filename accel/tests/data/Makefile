.PHONY: all clean

all: add.ptx add.cubin sub.ptx sub.cubin

clean:
	rm *.ptx *.cubin

%.ptx: %.cu
	nvcc -ptx $<

%.cubin: %.cu
	nvcc -cubin $<
