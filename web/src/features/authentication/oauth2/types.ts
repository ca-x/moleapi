import type {Pair} from "../../../shared/types";
export type OAuth2Grant="authorization_code"|"implicit"|"client_credentials"|"password"|"device_code";
export interface OAuth2Auth {
 grant:OAuth2Grant;authorization_url:string;token_url:string;device_url:string;revocation_url:string;introspection_url:string;redirect_url:string;
 client_id:string;client_secret:string;username:string;password:string;scopes:string[];pkce:boolean;client_auth:"basic"|"body";
 authorization_params:Pair[];token_params:Pair[];token_headers:Pair[];token_id:string|null;auto_refresh:boolean;location:"header"|"query";name:string;prefix:string;
}
export const oauth2Config=():OAuth2Auth=>({grant:"authorization_code",authorization_url:"",token_url:"",device_url:"",revocation_url:"",introspection_url:"",redirect_url:"",client_id:"",client_secret:"",username:"",password:"",scopes:[],pkce:true,client_auth:"basic",authorization_params:[],token_params:[],token_headers:[],token_id:null,auto_refresh:true,location:"header",name:"Authorization",prefix:"Bearer"});
export interface OAuth2Token {id:string;label:string;client_id:string;issuer:string;token_type:string;scopes:string[];created_at:number;expires_at:number|null;has_refresh_token:boolean;revoked:boolean;refreshing:boolean}
export interface OAuth2Flow {id:string;stage:string;authorization_url:string|null;verification_uri:string|null;user_code:string|null;expires_at:number;token_id:string|null;error:string|null}
