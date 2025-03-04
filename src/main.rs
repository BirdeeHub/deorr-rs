use wgpu::util::DeviceExt;
use pollster::block_on;
use std::time::Instant;

fn get_adapter() -> Option<wgpu::Adapter> {
    // Initialize GPU
    let instance = wgpu::Instance::default();

    let adapters = instance.enumerate_adapters(wgpu::Backends::all());

    if adapters.is_empty() {
        println!("No adapters found!");
    } else {
        for adapter in &adapters {
            println!("{:?}", adapter.get_info());
        }
    }

    adapters.clone()
        .into_iter()
        .find(|a| a.get_info().device_type == wgpu::DeviceType::DiscreteGpu) // Prefer discrete GPU
        .or_else(|| {
            // If no discrete GPU, try for integrated GPU
            adapters.iter().find(|a| a.get_info().device_type == wgpu::DeviceType::IntegratedGpu).cloned()
        })
        .or_else(|| {
            // If neither discrete nor integrated GPU, fall back to any available adapter
            println!("No discrete or integrated GPU found. Falling back to software rendering.");
            adapters.first().cloned() // Get the first available adapter
        })
}

async fn request_device(adapter: &wgpu::Adapter) -> Result<(wgpu::Device, wgpu::Queue),wgpu::RequestDeviceError> {
    adapter.request_device(&wgpu::DeviceDescriptor::default(), None).await
}

async fn run(adapter: &wgpu::Adapter, device: &wgpu::Device, queue: &wgpu::Queue, input_data: &[u32]) -> Vec<u32> {

    // Input data
    let input_len = input_data.len();
    if input_len == 0 {
        return vec![]
    }
    let buffer_size = (input_len * std::mem::size_of_val(input_data.first().unwrap_or(&0))) as wgpu::BufferAddress;

    // NOTE: input needs to be even factor or multiple of the COPY_BUFFER_ALIGNMENT
    // If input data length is not aligned, pad it
    let limits = adapter.limits();
    let copy_buffer_alignment: wgpu::BufferAddress = limits.min_storage_buffer_offset_alignment.into();
    let padding = (buffer_size.div_ceil(copy_buffer_alignment) * copy_buffer_alignment) - buffer_size;
    let mut padded_input_data = bytemuck::cast_slice(input_data).to_vec();
    padded_input_data.extend(vec![0u8; padding as usize]);

    // Create input buffer (READ-ONLY)
    let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Input Buffer"),
        contents: &padded_input_data,
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
    // TODO: figure out how to do generic alignment properly, only u32 works rn
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
    let result_data = bytemuck::cast_slice(&mapped_range).to_vec();

    // Unmap buffer
    drop(mapped_range);
    readback_buffer.unmap();
    result_data
}

fn main() {
    let Some(adapter) = get_adapter() else {
        println!("no gpu adapter found");
        return
    };

    let mut inputs = vec![vec![0; 1000];1000];
    inputs.iter_mut().for_each(|row| row.iter_mut().for_each(|v| { *v = rand::random_range(0..1000); }));
    let mut outputs = vec![vec![];1000];

    let begin = Instant::now();

    let Ok((device, queue)) = block_on(request_device(&adapter)) else {
        println!("Failed to request device");
        return
    };

    let start = Instant::now();

    for (i, input_data) in inputs.iter().enumerate() {
        outputs[i] = block_on(run(&adapter,&device,&queue,input_data));
    }

    println!("Total time: {:?}, Sort time: {:?}", begin.elapsed(), start.elapsed());

}
