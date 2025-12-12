export type ProjectOptions = {
    versions: string[];
    loaders: string[];
    slug?: string;
    id?: number;
    version_to_loaders?: Record<string, string[]>;
    loader_to_versions?: Record<string, string[]>;
};

