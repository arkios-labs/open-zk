fn main() {
    #[cfg(feature = "rebuild-guest")]
    {
        let guest_options = {
            let mut opts = risc0_build::GuestOptions::default();
            opts.features.push("risc0".to_string());

            // In debug mode or with debug-guest-build feature, use local
            // cross-compilation via rzup toolchain (faster, no Docker needed).
            // In release mode without debug-guest-build, use Docker for
            // reproducible builds.
            #[cfg(not(any(feature = "debug-guest-build", debug_assertions)))]
            let opts = {
                let mut opts = opts;
                opts.use_docker = Some(
                    risc0_build::DockerOptionsBuilder::default()
                        .root_dir({
                            // build/risc0/ → build/ → repo root
                            let cwd = std::env::current_dir().unwrap();
                            cwd.parent()
                                .unwrap()
                                .parent()
                                .map(|d| d.to_path_buf())
                                .unwrap()
                        })
                        .build()
                        .unwrap(),
                );
                opts
            };

            opts
        };

        risc0_build::embed_methods_with_options(std::collections::HashMap::from([(
            "guest-range-ethereum",
            guest_options,
        )]));
    }

    println!("cargo:rerun-if-changed=src");
}
