use hwloc::{CpuSet, ObjectType, Topology, TopologyFlag};
use std::thread;

fn main() {
    let topo = Topology::with_flags(vec![TopologyFlag::WholeSystem]);
    let cores = topo
        .objects_with_type(&ObjectType::Core)
        .unwrap_or_default();
    let mut socket_map: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    println!(
        "Found {} cores and {} package(s)",
        cores.len(),
        1, //packages.len()
    );
    for core in cores {
        // Only group by socket if it has a valid parent
        let socket_id = core
            .parent()
            .filter(|p| p.object_type() == ObjectType::Package)
            .map(|p| p.os_index())
            .unwrap_or(0); // fallback if no parent package

        if let Some(bitmap) = core.cpuset() {
            let pu_objs = topo.objects_with_type(&ObjectType::PU).unwrap();
            let max_pu = pu_objs.iter().map(|pu| pu.os_index()).max().unwrap_or(0);
            let pu_list: Vec<_> = (0..=max_pu).filter(|&i| bitmap.is_set(i)).collect();

            println!(
                "Core {} (socket {}): {:?}",
                core.logical_index(),
                socket_id,
                pu_list
            );

            socket_map
                .entry(socket_id)
                .or_default()
                .push(core.logical_index());
        }
    }

    for (socket_id, cores) in &socket_map {
        println!(
            "Socket {} has {} cores: {:?}",
            socket_id,
            cores.len(),
            cores
        );
    }
}

fn main2() {
    let topo = Topology::new();

    let cores = topo
        .objects_with_type(&ObjectType::Core)
        .unwrap_or_default();
    let packages = topo
        .objects_with_type(&ObjectType::Package)
        .unwrap_or_default();

    println!(
        "Found {} cores and {} package(s)",
        cores.len(),
        packages.len()
    );

    // Group cores by their parent package
    let mut socket_map: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();

    for core in cores {
        // Try to find the enclosing socket/package
        let socket_id = core
            .clone()
            .parent()
            .filter(|p| p.object_type() == ObjectType::Package)
            .map(|pkg| pkg.logical_index())
            .unwrap_or(0);

        if let Some(bitmap) = core.cpuset() {
            let pu_objs = topo.objects_with_type(&ObjectType::PU).unwrap();
            let max_pu = pu_objs.iter().map(|pu| pu.os_index()).max().unwrap_or(0);

            let pu_list: Vec<_> = (0..=max_pu).filter(|&i| bitmap.is_set(i)).collect();

            println!(
                "Core {} (socket {}): {:?}",
                core.logical_index(),
                socket_id,
                pu_list
            );
            socket_map
                .entry(socket_id as usize)
                .or_default()
                .push(core.logical_index() as usize);
        }
    }

    for (socket_id, cores) in socket_map {
        println!(
            "Socket {} has {} cores: {:?}",
            socket_id,
            cores.len(),
            cores
        );
    }
}
// fn main() {
//     let topo = Topology::new(); //with_flags(vec![TopologyFlag::WholeSystem]);

//     let sockets = topo.objects_with_type(&ObjectType::Package).unwrap();
//     println!("Found {} socket(s)", sockets.len());

//     for (s_id, socket) in sockets.iter().enumerate() {
//         println!("Socket {} has {} cores:", s_id, socket.arity());

//         for core in socket.children() {
//             if core.object_type() == ObjectType::Core {
//                 let bitmap = core.cpuset().unwrap();
//                 let pu_objs = topo.objects_with_type(&ObjectType::PU).unwrap();
//                 let max_pu_index = pu_objs.iter().map(|pu| pu.os_index()).max().unwrap_or(0);

//                 let mut pu_list: Vec<_> = Vec::new();
//                 for i in 0..=max_pu_index {
//                     if bitmap.is_set(i) {
//                         pu_list.push(i);
//                     }
//                 }

//                 // let pu_list = core
//                 //     .cpuset()
//                 //     .unwrap()
//                 //     .to_vec()
//                 //     .iter()
//                 //     .enumerate()
//                 //     .filter(|(_, b)| **b)
//                 //     .map(|(i, _)| i)
//                 //     .collect::<Vec<_>>();

//                 println!("  Core {}: {:?}", core.logical_index(), pu_list);
//             }
//         }
//     }

//     // Pin a thread to the first core of the first socket
//     let core_cpuset = sockets[0]
//         .children()
//         .iter()
//         .find(|c| c.object_type() == ObjectType::Core)
//         .and_then(|core| Some(core.cpuset().unwrap()));

//     // thread::spawn(move || {
//     //     let mut this_topo = Topology::new();
//     //     this_topo
//     //         .set_cpubind(core_cpuset, hwloc::CPUBIND_THREAD)
//     //         .unwrap();
//     //     println!("Pinned to core in socket 0");
//     //     std::thread::sleep(std::time::Duration::from_secs(2));
//     // })
//     // .join()
//     // .unwrap();
// }
