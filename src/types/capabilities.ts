import { ProtocolType } from "./connection";

export interface ProtocolCapabilities {
  protocol: ProtocolType;
  can_list_directory: boolean;
  can_stat_metadata: boolean;
  can_upload: boolean;
  can_download: boolean;
  can_create_file: boolean;
  can_create_directory: boolean;
  can_rename: boolean;
  can_delete_file: boolean;
  can_delete_directory: boolean;
  can_set_permissions: boolean;
  can_set_owner_group: boolean;
  can_elevate_sudo: boolean;
  can_preserve_timestamps: boolean;
  can_resume_transfer: boolean;
  can_symlink: boolean;
}
