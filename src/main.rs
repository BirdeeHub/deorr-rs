use wgpu::util::DeviceExt;
use pollster::block_on;

async fn run() {
    // Initialize GPU
    let instance = wgpu::Instance::default();

    // TODO: figure out how to detect this system dependent value
    const COPY_BUFFER_ALIGNMENT: wgpu::BufferAddress = 256;

    let adapters = instance.enumerate_adapters(wgpu::Backends::all());

    if adapters.is_empty() {
        println!("No adapters found!");
    } else {
        for adapter in &adapters {
            println!("{:?}", adapter.get_info());
        }
    }

    let adapter = adapters
        .iter()
        .find(|a| a.get_info().device_type == wgpu::DeviceType::DiscreteGpu) // Prefer discrete GPU
        .or_else(|| {
            // If no discrete GPU, try for integrated GPU
            adapters.iter().find(|a| a.get_info().device_type == wgpu::DeviceType::IntegratedGpu)
        })
        .or_else(|| {
            // If neither discrete nor integrated GPU, fall back to any available adapter
            println!("No discrete or integrated GPU found. Falling back to software rendering.");
            adapters.first()  // Get the first available adapter
        })
        .expect("Failed to find a suitable GPU adapter or fallback to software");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default(), None)
        .await
        .expect("Failed to create device");

    // Input data
    // TODO: figure out how to do generic alignment properly, only u32 works rn
    let input_data = vec![2, 5, 1, 7, 3, 3, 6, 8, 9, 4, 77, 33];
    let input_len = input_data.len();
    let buffer_size = (input_len * std::mem::size_of::<u32>()) as wgpu::BufferAddress;

    // NOTE: input needs to be even factor or multiple of the COPY_BUFFER_ALIGNMENT
    // If input data length is not aligned, pad it
    let mut padded_input_data = input_data.clone();
    let padding = (COPY_BUFFER_ALIGNMENT - (input_len as u64) % COPY_BUFFER_ALIGNMENT) % COPY_BUFFER_ALIGNMENT;
    padded_input_data.extend(vec![0u32; padding as usize]);

    // Create input buffer (READ-ONLY)
    let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Input Buffer"),
        contents: bytemuck::cast_slice(&padded_input_data),
        usage: wgpu::BufferUsages::STORAGE, // No COPY_DST, since we don’t modify it
    });

    // if you pass in length you dont need to remove padding
    let length_data = vec![input_len as u32];
    let length_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Length Buffer"),
        contents: bytemuck::cast_slice(&length_data),
        usage: wgpu::BufferUsages::STORAGE, // No COPY_SRC, since we don’t modify it
    });

    // Create output buffer (separate, writable)
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    // Create a readback buffer (for CPU access)
    let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Readback Buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Compute shader in WGSL
    // TODO: input buffer was misaligned when using floats, figure out how to align it properly
    let shader_code = r#"
        @group(0) @binding(0) var<storage, read> input_data: array<u32>;
        @group(0) @binding(1) var<storage, read_write> output_data: array<u32>;
        @group(0) @binding(2) var<storage, read> length_data: u32;
        @compute @workgroup_size(64)
        fn main(@builtin(global_invocation_id) id: vec3u) {
            let i = id.x;
            if (i >= length_data) {
                return;
            }
            let v = input_data[i];
            var finali = 0;
            for (var j = 0u; j < length_data; j++) {
                if (input_data[j] == v && j < i) {
                    finali += 1;
                }
                if (input_data[j] < v) {
                    finali += 1;
                }
            }
            output_data[finali] = v;
        }
    "#;

    // Create compute shader module
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Compute Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_code.into()),
    });

    // Define bind group layout
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true }, // Input: Read-only
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false }, // Output: Writable
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,  // New binding for length
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    // Create pipeline layout
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute Pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader_module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    // Create bind group
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: length_buffer.as_entire_binding(),
            },
        ],
        label: Some("Bind Group"),
    });

    // Create command encoder & compute pass
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Command Encoder"),
    });

    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Compute Pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&compute_pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups((input_len as u32).div_ceil(64), 1, 1);
    }

    // Copy results back to CPU-readable buffer
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback_buffer, 0, buffer_size);
    queue.submit(Some(encoder.finish()));

    // Read output data
    let buffer_slice = readback_buffer.slice(..);
    let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| sender.send(result).unwrap());

    device.poll(wgpu::Maintain::Wait);
    receiver.receive().await.unwrap().unwrap();

    // Get mapped buffer data
    let mapped_range = buffer_slice.get_mapped_range();
    let result_data: Vec<u32> = bytemuck::cast_slice(&mapped_range).to_vec();
    println!("Input:  {:?}", input_data);
    println!("Output: {:?}", result_data);

    // Unmap buffer
    drop(mapped_range);
    readback_buffer.unmap();
}

fn main() {
    block_on(run());
}
